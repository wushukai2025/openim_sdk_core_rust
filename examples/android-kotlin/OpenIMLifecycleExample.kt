package io.openim.example

class OpenIMLifecycleExample(
    private val bridge: OpenIMNativeBridge = OpenIMNativeBridge()
) {
    fun run(apiAddr: String, wsAddr: String, userID: String, token: String) {
        val handle = bridge.openimSessionCreate(apiAddr, wsAddr, OpenIMNativeBridge.PLATFORM_ANDROID)
        require(handle != 0L) { "OpenIM session create failed" }

        try {
            bridge.check(bridge.openimSessionInit(handle), handle)
            bridge.check(bridge.openimSessionLogin(handle, userID, token), handle)
            bridge.check(bridge.openimSessionLogout(handle), handle)
            bridge.check(bridge.openimSessionUninit(handle), handle)
        } finally {
            bridge.openimSessionDestroy(handle)
        }
    }
}

class OpenIMNativeBridge {
    external fun openimSessionCreate(apiAddr: String, wsAddr: String, platformID: Int): Long
    external fun openimSessionDestroy(handle: Long)
    external fun openimSessionInit(handle: Long): Int
    external fun openimSessionLogin(handle: Long, userID: String, token: String): Int
    external fun openimSessionLogout(handle: Long): Int
    external fun openimSessionUninit(handle: Long): Int
    external fun openimSessionLastError(handle: Long): String

    fun check(code: Int, handle: Long) {
        require(code == OK) { openimSessionLastError(handle) }
    }

    companion object {
        const val OK = 0
        const val PLATFORM_ANDROID = 2

        init {
            System.loadLibrary("openim_android_example")
        }
    }
}
