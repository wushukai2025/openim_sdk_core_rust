#ifndef OPENIM_FFI_H
#define OPENIM_FFI_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct OpenImFfiSession OpenImFfiSession;
typedef void (*OpenImFfiSessionEventCallback)(
    void *user_data,
    const char *event,
    const char *payload_json);

enum OpenImFfiCode {
  OPENIM_FFI_OK = 0,
  OPENIM_FFI_NULL = 1,
  OPENIM_FFI_INVALID_UTF8 = 2,
  OPENIM_FFI_INVALID_ARGS = 3,
  OPENIM_FFI_ERROR = 4
};

enum OpenImPlatform {
  OPENIM_PLATFORM_IOS = 1,
  OPENIM_PLATFORM_ANDROID = 2,
  OPENIM_PLATFORM_WINDOWS = 3,
  OPENIM_PLATFORM_MACOS = 4,
  OPENIM_PLATFORM_WEB = 5,
  OPENIM_PLATFORM_MINI_WEB = 6,
  OPENIM_PLATFORM_LINUX = 7,
  OPENIM_PLATFORM_ANDROID_PAD = 8,
  OPENIM_PLATFORM_IPAD = 9,
  OPENIM_PLATFORM_ADMIN = 10,
  OPENIM_PLATFORM_HARMONY_OS = 11
};

const char *openim_ffi_version(void);
const char *openim_native_callback_thread_policy(void);

OpenImFfiSession *openim_session_create(
    const char *api_addr,
    const char *ws_addr,
    int platform_id);
OpenImFfiSession *openim_session_create_with_data_dir(
    const char *api_addr,
    const char *ws_addr,
    int platform_id,
    const char *data_dir);
void openim_session_destroy(OpenImFfiSession *handle);

int openim_session_init(OpenImFfiSession *handle);
int openim_session_login(
    OpenImFfiSession *handle,
    const char *user_id,
    const char *token);
int openim_session_logout(OpenImFfiSession *handle);
int openim_session_uninit(OpenImFfiSession *handle);

int openim_session_state(const OpenImFfiSession *handle);
const char *openim_session_last_error(const OpenImFfiSession *handle);

/* event and payload_json are valid only during the callback call. */
uint64_t openim_session_register_listener(
    OpenImFfiSession *handle,
    OpenImFfiSessionEventCallback callback,
    void *user_data);
int openim_session_unregister_listener(
    OpenImFfiSession *handle,
    uint64_t listener_id);

#ifdef __cplusplus
}
#endif

#endif
