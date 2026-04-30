#include "openim_ffi.h"

#include <cstdint>
#include <jni.h>
#include <map>
#include <mutex>

struct AndroidListenerContext {
  JavaVM *vm;
  jobject listener;
};

struct ListenerKey {
  jlong handle;
  uint64_t listener_id;

  bool operator<(const ListenerKey &other) const {
    if (handle != other.handle) {
      return handle < other.handle;
    }
    return listener_id < other.listener_id;
  }
};

static std::mutex g_listener_mutex;
static std::map<ListenerKey, AndroidListenerContext *> g_listeners;

static OpenImFfiSession *from_handle(jlong handle) {
  return reinterpret_cast<OpenImFfiSession *>(handle);
}

static void delete_listener_context(JNIEnv *env, AndroidListenerContext *context) {
  if (context == nullptr) {
    return;
  }
  env->DeleteGlobalRef(context->listener);
  delete context;
}

static void on_android_session_event(void *user_data, const char *event, const char *payload_json) {
  auto *context = reinterpret_cast<AndroidListenerContext *>(user_data);
  if (context == nullptr) {
    return;
  }

  JNIEnv *env = nullptr;
  bool attached = false;
  if (context->vm->GetEnv(reinterpret_cast<void **>(&env), JNI_VERSION_1_6) != JNI_OK) {
    if (context->vm->AttachCurrentThread(&env, nullptr) != JNI_OK) {
      return;
    }
    attached = true;
  }

  jclass listener_class = env->GetObjectClass(context->listener);
  jmethodID on_event = env->GetMethodID(
      listener_class,
      "onEvent",
      "(Ljava/lang/String;Ljava/lang/String;)V");
  jstring event_string = env->NewStringUTF(event ? event : "");
  jstring payload_string = env->NewStringUTF(payload_json ? payload_json : "{}");
  env->CallVoidMethod(context->listener, on_event, event_string, payload_string);
  env->DeleteLocalRef(payload_string);
  env->DeleteLocalRef(event_string);
  env->DeleteLocalRef(listener_class);

  if (attached) {
    context->vm->DetachCurrentThread();
  }
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

extern "C" JNIEXPORT jlong JNICALL
Java_io_openim_example_OpenIMNativeBridge_openimSessionRegisterListener(
    JNIEnv *env,
    jobject,
    jlong handle,
    jobject listener) {
  if (listener == nullptr) {
    return 0;
  }

  JavaVM *vm = nullptr;
  if (env->GetJavaVM(&vm) != JNI_OK) {
    return 0;
  }

  auto *context = new AndroidListenerContext{
      vm,
      env->NewGlobalRef(listener),
  };
  if (context->listener == nullptr) {
    delete context;
    return 0;
  }

  uint64_t listener_id = openim_session_register_listener(
      from_handle(handle),
      on_android_session_event,
      context);
  if (listener_id == 0) {
    delete_listener_context(env, context);
    return 0;
  }

  std::lock_guard<std::mutex> lock(g_listener_mutex);
  g_listeners.insert({ListenerKey{handle, listener_id}, context});
  return static_cast<jlong>(listener_id);
}

extern "C" JNIEXPORT jint JNICALL
Java_io_openim_example_OpenIMNativeBridge_openimSessionUnregisterListener(
    JNIEnv *env,
    jobject,
    jlong handle,
    jlong listener_id) {
  int code = openim_session_unregister_listener(
      from_handle(handle),
      static_cast<uint64_t>(listener_id));
  if (code != OPENIM_FFI_OK) {
    return code;
  }

  AndroidListenerContext *context = nullptr;
  {
    std::lock_guard<std::mutex> lock(g_listener_mutex);
    ListenerKey key{handle, static_cast<uint64_t>(listener_id)};
    auto it = g_listeners.find(key);
    if (it != g_listeners.end()) {
      context = it->second;
      g_listeners.erase(it);
    }
  }
  delete_listener_context(env, context);
  return code;
}

extern "C" JNIEXPORT jstring JNICALL
Java_io_openim_example_OpenIMNativeBridge_openimSessionLastError(JNIEnv *env, jobject, jlong handle) {
  const char *error = openim_session_last_error(from_handle(handle));
  return env->NewStringUTF(error ? error : "");
}
