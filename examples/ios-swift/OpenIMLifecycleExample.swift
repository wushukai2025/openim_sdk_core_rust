import Foundation

enum OpenIMLifecycleError: Error {
    case createFailed
    case nativeError(Int32, String)
}

final class OpenIMLifecycleExample {
    private var handle: OpaquePointer?

    deinit {
        if let handle {
            openim_session_destroy(handle)
        }
    }

    func run(apiAddr: String, wsAddr: String, userID: String, token: String) throws {
        let session = try apiAddr.withCString { api in
            try wsAddr.withCString { ws in
                guard let session = openim_session_create(api, ws, OPENIM_PLATFORM_IOS) else {
                    throw OpenIMLifecycleError.createFailed
                }
                return session
            }
        }
        handle = session

        try check(openim_session_init(session), session)
        try userID.withCString { user in
            try token.withCString { authToken in
                try check(openim_session_login(session, user, authToken), session)
            }
        }
        try check(openim_session_logout(session), session)
        try check(openim_session_uninit(session), session)

        openim_session_destroy(session)
        handle = nil
    }

    private func check(_ code: Int32, _ session: OpaquePointer) throws {
        guard code == OPENIM_FFI_OK else {
            let message = openim_session_last_error(session).map { String(cString: $0) } ?? ""
            throw OpenIMLifecycleError.nativeError(code, message)
        }
    }
}
