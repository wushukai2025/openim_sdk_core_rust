#include "openim_ffi.h"

#include <jni.h>

static OpenImFfiSession *from_handle(jlong handle) {
  return reinterpret_cast<OpenImFfiSession *>(handle);
}

extern "C" JNIEXPORT jlong JNICALL
Java_io_openim_example_OpenIMNativeBridge_openimSessionCreate(
    JNIEnv *env,
    jobject,
    jstring api_addr,
    jstring ws_addr,
    jint platform_id) {
  const char *api = env->GetStringUTFChars(api_addr, nullptr);
  const char *ws = env->GetStringUTFChars(ws_addr, nullptr);
  if (api == nullptr || ws == nullptr) {
    if (api != nullptr) {
      env->ReleaseStringUTFChars(api_addr, api);
    }
    if (ws != nullptr) {
      env->ReleaseStringUTFChars(ws_addr, ws);
    }
    return 0;
  }
  OpenImFfiSession *session = openim_session_create(api, ws, platform_id);
  env->ReleaseStringUTFChars(api_addr, api);
  env->ReleaseStringUTFChars(ws_addr, ws);
  return reinterpret_cast<jlong>(session);
}

extern "C" JNIEXPORT void JNICALL
Java_io_openim_example_OpenIMNativeBridge_openimSessionDestroy(JNIEnv *, jobject, jlong handle) {
  openim_session_destroy(from_handle(handle));
}

extern "C" JNIEXPORT jint JNICALL
Java_io_openim_example_OpenIMNativeBridge_openimSessionInit(JNIEnv *, jobject, jlong handle) {
  return openim_session_init(from_handle(handle));
}

extern "C" JNIEXPORT jint JNICALL
Java_io_openim_example_OpenIMNativeBridge_openimSessionLogin(
    JNIEnv *env,
    jobject,
    jlong handle,
    jstring user_id,
    jstring token) {
  const char *user = env->GetStringUTFChars(user_id, nullptr);
  const char *tok = env->GetStringUTFChars(token, nullptr);
  if (user == nullptr || tok == nullptr) {
    if (user != nullptr) {
      env->ReleaseStringUTFChars(user_id, user);
    }
    if (tok != nullptr) {
      env->ReleaseStringUTFChars(token, tok);
    }
    return OPENIM_FFI_NULL;
  }
  int code = openim_session_login(from_handle(handle), user, tok);
  env->ReleaseStringUTFChars(user_id, user);
  env->ReleaseStringUTFChars(token, tok);
  return code;
}

extern "C" JNIEXPORT jint JNICALL
Java_io_openim_example_OpenIMNativeBridge_openimSessionLogout(JNIEnv *, jobject, jlong handle) {
  return openim_session_logout(from_handle(handle));
}

extern "C" JNIEXPORT jint JNICALL
Java_io_openim_example_OpenIMNativeBridge_openimSessionUninit(JNIEnv *, jobject, jlong handle) {
  return openim_session_uninit(from_handle(handle));
}

extern "C" JNIEXPORT jstring JNICALL
Java_io_openim_example_OpenIMNativeBridge_openimSessionLastError(JNIEnv *env, jobject, jlong handle) {
  const char *error = openim_session_last_error(from_handle(handle));
  return env->NewStringUTF(error ? error : "");
}
