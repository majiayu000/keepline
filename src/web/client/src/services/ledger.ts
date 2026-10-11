import { ledgerRequest } from "./api";

export async function ledgerApi<T>(
  path: string,
  method = "GET",
  data?: unknown,
): Promise<T> {
  const response = await ledgerRequest<T>(path, {
    method,
    ...(data === undefined ? {} : { body: JSON.stringify(data) }),
  });
  if (!response.success) throw new Error(response.error ?? "请求失败");
  return response.data as T;
}
