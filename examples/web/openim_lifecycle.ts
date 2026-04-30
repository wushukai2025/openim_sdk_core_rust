import init, {
  mapSessionEventToGoListeners,
  OpenImWasmSession,
} from "../../pkg/openim_wasm";

export async function runOpenIMLifecycle(
  apiAddr: string,
  wsAddr: string,
  userID: string,
  token: string,
): Promise<number> {
  await init();

  const session = new OpenImWasmSession(apiAddr, wsAddr, 5);
  const events: Array<{
    event: string;
    payload: unknown;
    goListenerDispatches: unknown;
  }> = [];
  const listenerID = session.addListener((event: string, payloadJson: string) => {
    events.push({
      event,
      payload: JSON.parse(payloadJson),
      goListenerDispatches: JSON.parse(
        mapSessionEventToGoListeners(event, payloadJson),
      ),
    });
  });

  session.init();
  session.login(userID, token);
  session.logout();
  session.uninit();
  session.removeListener(listenerID);

  return session.stateCode();
}
