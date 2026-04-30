import init, { OpenImWasmSession } from "../../pkg/openim_wasm";

export async function runOpenIMLifecycle(
  apiAddr: string,
  wsAddr: string,
  userID: string,
  token: string,
): Promise<number> {
  await init();

  const session = new OpenImWasmSession(apiAddr, wsAddr, 5);
  session.init();
  session.login(userID, token);
  session.logout();
  session.uninit();

  return session.stateCode();
}
