import { session } from "electron";
import type { Session, WebContents } from "electron";

const GRANTED = new Set(["clipboard-sanitized-write"]);
const configured = new WeakSet<Session>();
const clipboardReaders = new WeakSet<WebContents>();

export function allowClipboardRead(contents: WebContents): void {
  clipboardReaders.add(contents);
}

function granted(contents: WebContents | null, permission: string): boolean {
  return (
    GRANTED.has(permission) ||
    (permission === "clipboard-read" && contents !== null && clipboardReaders.has(contents))
  );
}

export function configureBrowserSession(): Session {
  const target = browserSession();
  if (configured.has(target)) return target;
  configured.add(target);
  target.setPermissionRequestHandler((contents, permission, callback) => {
    callback(granted(contents, permission));
  });
  target.setPermissionCheckHandler((contents, permission) => granted(contents, permission));
  return target;
}

export function browserSession(): Session {
  return session.defaultSession;
}
