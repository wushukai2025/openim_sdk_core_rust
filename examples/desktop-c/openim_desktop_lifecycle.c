#include "openim_ffi.h"

#include <stdio.h>
#include <stdlib.h>

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

static const char *env_or_default(const char *name, const char *fallback) {
  const char *value = getenv(name);
  return (value && value[0] != '\0') ? value : fallback;
}

int main(void) {
  const char *api_addr = env_or_default("OPENIM_API_ADDR", "https://api.openim.test");
  const char *ws_addr = env_or_default("OPENIM_WS_ADDR", "wss://ws.openim.test");
  const char *user_id = env_or_default("OPENIM_USER_ID", "u1");
  const char *token = env_or_default("OPENIM_TOKEN", "token");
  const char *data_dir = getenv("OPENIM_DATA_DIR");

  OpenImFfiSession *session = openim_session_create_with_data_dir(
      api_addr,
      ws_addr,
      OPENIM_PLATFORM_MACOS,
      data_dir);
  if (!session) {
    fprintf(stderr, "OpenIM session create failed\n");
    return 1;
  }

  uint64_t listener_id = openim_session_register_listener(session, on_session_event, NULL);
  if (listener_id == 0) {
    int result = check_openim(OPENIM_FFI_ERROR, session);
    openim_session_destroy(session);
    return result == 0 ? 0 : 1;
  }

  int code = openim_session_init(session);
  if (code == OPENIM_FFI_OK) {
    code = openim_session_login(session, user_id, token);
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
