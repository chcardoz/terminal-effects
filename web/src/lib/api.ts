export async function request<T>(path: string, body?: unknown): Promise<T> {
  const response = await fetch(path, body === undefined
    ? { cache: "no-store" }
    : {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
  const payload = await response.json().catch(() => ({ error: response.statusText }));
  if (!response.ok) {
    const message = typeof payload?.error === "string" ? payload.error : `Request failed (${response.status})`;
    throw new Error(message);
  }
  return payload as T;
}
