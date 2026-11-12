const TOKEN_KEY = "xiezhi_auth_token";
const USER_KEY = "xiezhi_username";

function readStorage(key: string): string | null {
  if (typeof window === "undefined") return null;
  try {
    return sessionStorage.getItem(key) ?? localStorage.getItem(key);
  } catch {
    return null;
  }
}

export function getToken(): string | null {
  return readStorage(TOKEN_KEY);
}

export function getUsername(): string | null {
  return readStorage(USER_KEY);
}

export function setSession(token: string, username: string, remember = true) {
  clearSession();
  const storage = remember ? localStorage : sessionStorage;
  storage.setItem(TOKEN_KEY, token);
  storage.setItem(USER_KEY, username);
}

export function clearSession() {
  try {
    localStorage.removeItem(TOKEN_KEY);
    localStorage.removeItem(USER_KEY);
    sessionStorage.removeItem(TOKEN_KEY);
    sessionStorage.removeItem(USER_KEY);
  } catch {
    // storage may be unavailable (private mode / policy)
  }
}

export class ApiError extends Error {
  status: number;
  constructor(message: string, status: number) {
    super(message);
    this.status = status;
  }
}

export async function api<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const token = getToken();
  const headers = new Headers(options.headers || {});
  const body = options.body;
  if (
    !headers.has("Content-Type") &&
    body &&
    typeof body === "string"
  ) {
    headers.set("Content-Type", "application/json");
  }
  if (token) headers.set("Authorization", `Bearer ${token}`);

  let res: Response;
  try {
    res = await fetch(path, { ...options, headers });
  } catch {
    throw new ApiError("网络请求失败，请检查网络连接", 0);
  }
  if (res.status === 401) {
    clearSession();
    if (typeof window !== "undefined" && !window.location.pathname.startsWith("/login")) {
      window.location.href = "/login";
    }
    throw new ApiError("unauthorized", 401);
  }

  const raw = await res.text();
  let data: any = {};
  if (raw) {
    try {
      data = JSON.parse(raw);
    } catch {
      if (!res.ok) {
        throw new ApiError(`请求失败 (${res.status})`, res.status);
      }
      throw new ApiError("响应格式无效", res.status);
    }
  }

  if (!res.ok) {
    throw new ApiError(data?.message || `请求失败 (${res.status})`, res.status);
  }
  return data as T;
}
