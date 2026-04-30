#include "openim_ffi.h"

#include <stdio.h>

static int check_openim(int code, OpenImFfiSession *session) {
  if (code == OPENIM_FFI_OK) {
    return 0;
  }

  const char *message = session ? openim_session_last_error(session) : "missing session";
  fprintf(stderr, "OpenIM lifecycle failed: %s\n", message ? message : "unknown error");
  return code;
}

static void on_session_event(void *user_data, const char *event, const char *payload_json) {
  (void)user_data;
  printf("OpenIM session event: %s %s\n", event, payload_json);
}

int main(void) {
  OpenImFfiSession *session = openim_session_create(
      "https://api.openim.test",
      "wss://ws.openim.test",
      OPENIM_PLATFORM_LINUX);
  if (!session) {
    fprintf(stderr, "OpenIM session create failed\n");
    return 1;
  }

  uint64_t listener_id = openim_session_register_listener(session, on_session_event, NULL);
  if (listener_id == 0) {
    return check_openim(OPENIM_FFI_ERROR, session);
  }

  int code = openim_session_init(session);
  if (code == OPENIM_FFI_OK) {
    code = openim_session_login(session, "u1", "token");
  }
  if (code == OPENIM_FFI_OK) {
    code = openim_session_logout(session);
  }
  if (code == OPENIM_FFI_OK) {
    code = openim_session_uninit(session);
  }
  if (code == OPENIM_FFI_OK) {
    code = openim_session_unregister_listener(session, listener_id);
  }

  int result = check_openim(code, session);
  openim_session_destroy(session);
  return result == 0 ? 0 : 1;
}
