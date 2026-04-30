package main

import (
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"

	openim "github.com/openimsdk/openim-sdk-core/v3/open_im_sdk"
	"github.com/openimsdk/openim-sdk-core/v3/sdk_struct"
)

type config struct {
	apiAddr       string
	wsAddr        string
	userID        string
	token         string
	dataDir       string
	operationID   string
	platformID    int
	logLevel      int
	timeout       time.Duration
	waitAfterSync time.Duration
	sendTo        string
	sendText      string
	uploadReq     string
	requireMsg    bool
}

type replayEvent struct {
	Scenario string `json:"scenario"`
	Listener string `json:"listener"`
	Method   string `json:"method"`
	Payload  any    `json:"payload"`
}

type recorder struct {
	mu     sync.Mutex
	events []replayEvent
}

func (r *recorder) emit(scenario string, listener string, method string, payload any) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.events = append(r.events, replayEvent{
		Scenario: scenario,
		Listener: listener,
		Method:   method,
		Payload:  payload,
	})
}

func (r *recorder) writeJSONL(w io.Writer) error {
	r.mu.Lock()
	events := append([]replayEvent(nil), r.events...)
	r.mu.Unlock()

	encoder := json.NewEncoder(w)
	for _, event := range events {
		if err := encoder.Encode(event); err != nil {
			return err
		}
	}
	return nil
}

type resultWaiter struct {
	once sync.Once
	ch   chan error
}

func newResultWaiter() *resultWaiter {
	return &resultWaiter{ch: make(chan error, 1)}
}

func (w *resultWaiter) success() {
	w.once.Do(func() {
		w.ch <- nil
	})
}

func (w *resultWaiter) failure(err error) {
	w.once.Do(func() {
		w.ch <- err
	})
}

func (w *resultWaiter) wait(label string, timeout time.Duration) error {
	timer := time.NewTimer(timeout)
	defer timer.Stop()

	select {
	case err := <-w.ch:
		if err != nil {
			return err
		}
		return nil
	case <-timer.C:
		return fmt.Errorf("timed out waiting for %s after %s", label, timeout)
	}
}

type baseCallback struct {
	rec      *recorder
	scenario string
	done     *resultWaiter
}

func (c *baseCallback) OnSuccess(data string) {
	c.rec.emit(c.scenario, "Base", "OnSuccess", map[string]any{"data": data})
	c.done.success()
}

func (c *baseCallback) OnError(errCode int32, errMsg string) {
	c.rec.emit(c.scenario, "Base", "OnError", map[string]any{
		"errCode": errCode,
		"errMsg":  errMsg,
	})
	c.done.failure(fmt.Errorf("%s failed: %d %s", c.scenario, errCode, errMsg))
}

type connListener struct {
	rec *recorder
}

func (l *connListener) emit(method string, payload any, loginSync bool) {
	l.rec.emit("connection_status", "OnConnListener", method, payload)
	if loginSync {
		l.rec.emit("login_sync_message", "OnConnListener", method, payload)
	}
}

func (l *connListener) OnConnecting() {
	l.emit("OnConnecting", nil, true)
}

func (l *connListener) OnConnectSuccess() {
	l.emit("OnConnectSuccess", nil, true)
}

func (l *connListener) OnConnectFailed(errCode int32, errMsg string) {
	l.emit("OnConnectFailed", map[string]any{"errCode": errCode, "errMsg": errMsg}, false)
}

func (l *connListener) OnKickedOffline() {
	l.emit("OnKickedOffline", nil, false)
}

func (l *connListener) OnUserTokenExpired() {
	l.emit("OnUserTokenExpired", nil, false)
}

func (l *connListener) OnUserTokenInvalid(errMsg string) {
	l.emit("OnUserTokenInvalid", map[string]any{"errMsg": errMsg}, false)
}

type conversationListener struct {
	rec      *recorder
	syncDone *resultWaiter
}

func (l *conversationListener) OnSyncServerStart(reinstalled bool) {
	l.rec.emit("login_sync_message", "OnConversationListener", "OnSyncServerStart", map[string]any{"reinstalled": reinstalled})
}

func (l *conversationListener) OnSyncServerFinish(reinstalled bool) {
	l.rec.emit("login_sync_message", "OnConversationListener", "OnSyncServerFinish", map[string]any{"reinstalled": reinstalled})
	l.syncDone.success()
}

func (l *conversationListener) OnSyncServerProgress(progress int) {
	l.rec.emit("login_sync_message", "OnConversationListener", "OnSyncServerProgress", map[string]any{"progress": progress})
}

func (l *conversationListener) OnSyncServerFailed(reinstalled bool) {
	l.rec.emit("login_sync_message", "OnConversationListener", "OnSyncServerFailed", map[string]any{"reinstalled": reinstalled})
	l.syncDone.failure(errors.New("login sync failed"))
}

func (l *conversationListener) OnNewConversation(conversationList string) {
	payload := map[string]any{"conversationList": conversationList}
	l.rec.emit("login_sync_message", "OnConversationListener", "OnNewConversation", payload)
}

func (l *conversationListener) OnConversationChanged(conversationList string) {
	payload := map[string]any{"conversationList": conversationList}
	l.rec.emit("login_sync_message", "OnConversationListener", "OnConversationChanged", payload)
	l.rec.emit("message_arrival", "OnConversationListener", "OnConversationChanged", payload)
}

func (l *conversationListener) OnTotalUnreadMessageCountChanged(totalUnreadCount int32) {
	payload := map[string]any{"totalUnreadCount": totalUnreadCount}
	l.rec.emit("login_sync_message", "OnConversationListener", "OnTotalUnreadMessageCountChanged", payload)
	l.rec.emit("message_arrival", "OnConversationListener", "OnTotalUnreadMessageCountChanged", payload)
}

func (l *conversationListener) OnConversationUserInputStatusChanged(change string) {
	l.rec.emit("conversation", "OnConversationListener", "OnConversationUserInputStatusChanged", map[string]any{"change": change})
}

type advancedMsgListener struct {
	rec     *recorder
	msgDone *resultWaiter
}

func (l *advancedMsgListener) OnRecvNewMessage(message string) {
	l.rec.emit("message_arrival", "OnAdvancedMsgListener", "OnRecvNewMessage", map[string]any{"message": message})
	l.msgDone.success()
}

func (l *advancedMsgListener) OnRecvC2CReadReceipt(msgReceiptList string) {
	l.rec.emit("advanced_message", "OnAdvancedMsgListener", "OnRecvC2CReadReceipt", map[string]any{"msgReceiptList": msgReceiptList})
}

func (l *advancedMsgListener) OnNewRecvMessageRevoked(messageRevoked string) {
	l.rec.emit("advanced_message", "OnAdvancedMsgListener", "OnNewRecvMessageRevoked", map[string]any{"messageRevoked": messageRevoked})
}

func (l *advancedMsgListener) OnRecvOfflineNewMessage(message string) {
	l.rec.emit("login_sync_message", "OnAdvancedMsgListener", "OnRecvOfflineNewMessage", map[string]any{"message": message})
}

func (l *advancedMsgListener) OnMsgDeleted(message string) {
	l.rec.emit("advanced_message", "OnAdvancedMsgListener", "OnMsgDeleted", map[string]any{"message": message})
}

func (l *advancedMsgListener) OnRecvOnlineOnlyMessage(message string) {
	l.rec.emit("advanced_message", "OnAdvancedMsgListener", "OnRecvOnlineOnlyMessage", map[string]any{"message": message})
}

type sendMsgCallback struct {
	rec  *recorder
	done *resultWaiter
}

func (c *sendMsgCallback) OnProgress(progress int) {
	c.rec.emit("message_send", "SendMsgCallBack", "OnProgress", map[string]any{"progress": progress})
}

func (c *sendMsgCallback) OnSuccess(data string) {
	c.rec.emit("message_send", "SendMsgCallBack", "OnSuccess", map[string]any{"data": data})
	c.done.success()
}

func (c *sendMsgCallback) OnError(errCode int32, errMsg string) {
	c.rec.emit("message_send", "SendMsgCallBack", "OnError", map[string]any{
		"errCode": errCode,
		"errMsg":  errMsg,
	})
	c.done.failure(fmt.Errorf("send message failed: %d %s", errCode, errMsg))
}

type uploadFileCallback struct {
	rec *recorder
}

func (c *uploadFileCallback) Open(size int64) {
	c.rec.emit("file_upload_progress", "UploadFileCallback", "Open", map[string]any{"size": size})
}

func (c *uploadFileCallback) PartSize(partSize int64, num int) {
	c.rec.emit("file_upload_progress", "UploadFileCallback", "PartSize", map[string]any{
		"partSize": partSize,
		"num":      num,
	})
}

func (c *uploadFileCallback) HashPartProgress(index int, size int64, partHash string) {
	c.rec.emit("file_upload_progress", "UploadFileCallback", "HashPartProgress", map[string]any{
		"index":    index,
		"size":     size,
		"partHash": partHash,
	})
}

func (c *uploadFileCallback) HashPartComplete(partsHash string, fileHash string) {
	c.rec.emit("file_upload_progress", "UploadFileCallback", "HashPartComplete", map[string]any{
		"partsHash": partsHash,
		"fileHash":  fileHash,
	})
}

func (c *uploadFileCallback) UploadID(uploadID string) {
	c.rec.emit("file_upload_progress", "UploadFileCallback", "UploadID", map[string]any{"uploadID": uploadID})
}

func (c *uploadFileCallback) UploadPartComplete(index int, partSize int64, partHash string) {
	c.rec.emit("file_upload_progress", "UploadFileCallback", "UploadPartComplete", map[string]any{
		"index":    index,
		"partSize": partSize,
		"partHash": partHash,
	})
}

func (c *uploadFileCallback) UploadComplete(fileSize int64, streamSize int64, storageSize int64) {
	c.rec.emit("file_upload_progress", "UploadFileCallback", "UploadComplete", map[string]any{
		"fileSize":    fileSize,
		"streamSize":  streamSize,
		"storageSize": storageSize,
	})
}

func (c *uploadFileCallback) Complete(size int64, url string, typ int) {
	c.rec.emit("file_upload_progress", "UploadFileCallback", "Complete", map[string]any{
		"size": size,
		"url":  url,
		"type": typ,
	})
}

func main() {
	cfg, err := parseConfig()
	if err != nil {
		fail(err)
	}

	rec := &recorder{}
	restoreStdout, err := redirectStdoutToStderr()
	if err != nil {
		fail(err)
	}
	runErr := run(cfg, rec)
	if err := restoreStdout(); err != nil && runErr == nil {
		runErr = err
	}
	if runErr != nil {
		fail(runErr)
	}
	if err := rec.writeJSONL(os.Stdout); err != nil {
		fail(err)
	}
}

func parseConfig() (config, error) {
	apiAddr := flag.String("api-addr", envString("OPENIM_API_ADDR", ""), "OpenIM HTTP API address")
	wsAddr := flag.String("ws-addr", envString("OPENIM_WS_ADDR", ""), "OpenIM WebSocket address")
	userID := flag.String("user-id", envString("OPENIM_USER_ID", ""), "login user ID")
	token := flag.String("token", envString("OPENIM_TOKEN", ""), "login token")
	dataDir := flag.String("data-dir", envString("OPENIM_DATA_DIR", ""), "SDK data directory")
	operationID := flag.String("operation-id", envString("OPENIM_OPERATION_ID", "phase0-go-replay"), "operation ID prefix")
	platformID := flag.Int("platform-id", envInt("OPENIM_PLATFORM_ID", 5), "OpenIM platform ID")
	logLevel := flag.Int("log-level", envInt("OPENIM_LOG_LEVEL", 3), "OpenIM SDK log level")
	timeout := flag.Duration("timeout", envDuration("OPENIM_REPLAY_TIMEOUT", 60*time.Second), "operation timeout")
	waitAfterSync := flag.Duration("wait-after-sync", envDuration("OPENIM_REPLAY_WAIT_AFTER_SYNC", 10*time.Second), "extra wait time after login sync")
	sendTo := flag.String("send-to", envString("OPENIM_SEND_TO_USER_ID", ""), "optional user ID to send a text message to")
	sendText := flag.String("send-text", envString("OPENIM_SEND_TEXT", "phase0 replay message"), "optional text message content")
	uploadReq := flag.String("upload-req", envString("OPENIM_UPLOAD_REQ", ""), "optional UploadFile request JSON")
	requireMsg := flag.Bool("require-message", envBool("OPENIM_REQUIRE_MESSAGE", false), "require OnRecvNewMessage before logout")
	flag.Parse()

	cfg := config{
		apiAddr:       strings.TrimSpace(*apiAddr),
		wsAddr:        strings.TrimSpace(*wsAddr),
		userID:        strings.TrimSpace(*userID),
		token:         strings.TrimSpace(*token),
		dataDir:       strings.TrimSpace(*dataDir),
		operationID:   strings.TrimSpace(*operationID),
		platformID:    *platformID,
		logLevel:      *logLevel,
		timeout:       *timeout,
		waitAfterSync: *waitAfterSync,
		sendTo:        strings.TrimSpace(*sendTo),
		sendText:      *sendText,
		uploadReq:     strings.TrimSpace(*uploadReq),
		requireMsg:    *requireMsg,
	}
	if cfg.apiAddr == "" {
		return cfg, errors.New("missing --api-addr or OPENIM_API_ADDR")
	}
	if cfg.wsAddr == "" {
		return cfg, errors.New("missing --ws-addr or OPENIM_WS_ADDR")
	}
	if cfg.userID == "" {
		return cfg, errors.New("missing --user-id or OPENIM_USER_ID")
	}
	if cfg.token == "" {
		return cfg, errors.New("missing --token or OPENIM_TOKEN")
	}
	if cfg.operationID == "" {
		return cfg, errors.New("missing --operation-id or OPENIM_OPERATION_ID")
	}
	if cfg.timeout <= 0 {
		return cfg, errors.New("--timeout must be positive")
	}
	if cfg.waitAfterSync < 0 {
		return cfg, errors.New("--wait-after-sync cannot be negative")
	}
	if cfg.dataDir == "" {
		cfg.dataDir = filepath.Join(os.TempDir(), "openim-phase0-go-replay", cfg.userID)
	}
	return cfg, nil
}

func run(cfg config, rec *recorder) error {
	if err := os.MkdirAll(cfg.dataDir, 0o755); err != nil {
		return fmt.Errorf("create data dir: %w", err)
	}

	syncDone := newResultWaiter()
	msgDone := newResultWaiter()
	loginDone := newResultWaiter()
	logoutDone := newResultWaiter()

	imConfig := sdk_struct.IMConfig{
		SystemType:          "go-phase0-replay",
		PlatformID:          int32(cfg.platformID),
		ApiAddr:             cfg.apiAddr,
		WsAddr:              cfg.wsAddr,
		DataDir:             cfg.dataDir,
		LogLevel:            uint32(cfg.logLevel),
		IsLogStandardOutput: false,
		LogFilePath:         filepath.Join(cfg.dataDir, "logs"),
		LogRemainCount:      1,
	}
	configJSON, err := json.Marshal(imConfig)
	if err != nil {
		return err
	}

	if ok := openim.InitSDK(&connListener{rec: rec}, op(cfg, "init"), string(configJSON)); !ok {
		return errors.New("InitSDK returned false")
	}
	defer openim.UnInitSDK(op(cfg, "uninit"))

	openim.SetConversationListener(&conversationListener{rec: rec, syncDone: syncDone})
	openim.SetAdvancedMsgListener(&advancedMsgListener{rec: rec, msgDone: msgDone})
	openim.Login(&baseCallback{rec: rec, scenario: "login", done: loginDone}, op(cfg, "login"), cfg.userID, cfg.token)
	if err := loginDone.wait("Login callback", cfg.timeout); err != nil {
		return err
	}
	if err := syncDone.wait("login sync callback", cfg.timeout); err != nil {
		return err
	}

	if cfg.sendTo != "" {
		sendDone := newResultWaiter()
		message := openim.CreateTextMessage(op(cfg, "create-text"), cfg.sendText)
		if message == "" {
			return errors.New("CreateTextMessage returned empty message")
		}
		openim.SendMessage(&sendMsgCallback{rec: rec, done: sendDone}, op(cfg, "send-message"), message, cfg.sendTo, "", "", false)
		if err := sendDone.wait("SendMessage callback", cfg.timeout); err != nil {
			return err
		}
	}

	if cfg.uploadReq != "" {
		uploadDone := newResultWaiter()
		openim.UploadFile(
			&baseCallback{rec: rec, scenario: "file_upload_result", done: uploadDone},
			op(cfg, "upload-file"),
			cfg.uploadReq,
			&uploadFileCallback{rec: rec},
		)
		if err := uploadDone.wait("UploadFile callback", cfg.timeout); err != nil {
			return err
		}
	}

	if cfg.requireMsg {
		if err := msgDone.wait("OnRecvNewMessage callback", cfg.timeout); err != nil {
			return err
		}
	} else if cfg.waitAfterSync > 0 {
		time.Sleep(cfg.waitAfterSync)
	}

	openim.Logout(&baseCallback{rec: rec, scenario: "logout", done: logoutDone}, op(cfg, "logout"))
	if err := logoutDone.wait("Logout callback", cfg.timeout); err != nil {
		return err
	}
	return nil
}

func op(cfg config, name string) string {
	return cfg.operationID + "-" + name
}

func redirectStdoutToStderr() (func() error, error) {
	stdoutFD := int(os.Stdout.Fd())
	stderrFD := int(os.Stderr.Fd())
	savedFD, err := syscall.Dup(stdoutFD)
	if err != nil {
		return nil, fmt.Errorf("dup stdout: %w", err)
	}
	if err := syscall.Dup2(stderrFD, stdoutFD); err != nil {
		_ = syscall.Close(savedFD)
		return nil, fmt.Errorf("redirect stdout to stderr: %w", err)
	}
	return func() error {
		defer syscall.Close(savedFD)
		if err := syscall.Dup2(savedFD, stdoutFD); err != nil {
			return fmt.Errorf("restore stdout: %w", err)
		}
		return nil
	}, nil
}

func envString(name string, fallback string) string {
	if value := os.Getenv(name); value != "" {
		return value
	}
	return fallback
}

func envInt(name string, fallback int) int {
	value := os.Getenv(name)
	if value == "" {
		return fallback
	}
	parsed, err := strconv.Atoi(value)
	if err != nil {
		fail(fmt.Errorf("invalid %s: %w", name, err))
	}
	return parsed
}

func envDuration(name string, fallback time.Duration) time.Duration {
	value := os.Getenv(name)
	if value == "" {
		return fallback
	}
	parsed, err := time.ParseDuration(value)
	if err != nil {
		fail(fmt.Errorf("invalid %s: %w", name, err))
	}
	return parsed
}

func envBool(name string, fallback bool) bool {
	value := strings.TrimSpace(strings.ToLower(os.Getenv(name)))
	if value == "" {
		return fallback
	}
	switch value {
	case "1", "true", "yes", "y", "on":
		return true
	case "0", "false", "no", "n", "off":
		return false
	default:
		fail(fmt.Errorf("invalid %s: expected boolean", name))
	}
	return fallback
}

func fail(err error) {
	fmt.Fprintln(os.Stderr, err)
	os.Exit(1)
}
