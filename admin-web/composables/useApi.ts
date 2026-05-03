import type { FetchOptions } from 'ofetch'
import { ApiError, type ApiErrorBody } from '~/types/api'

/**
 * Returns a $fetch instance preconfigured for the argus API: base URL from
 * runtime config, credentials always included so the session cookie travels,
 * and JSON errors normalized into ApiError.
 */
export function useApi() {
  const config = useRuntimeConfig()
  const baseURL = config.public.apiBaseUrl

  return $fetch.create({
    baseURL,
    credentials: 'include',
    onResponseError({ response }) {
      const body = response._data as ApiErrorBody | undefined
      const code = body?.error?.code ?? 'UNKNOWN'
      const message = body?.error?.message ?? response.statusText ?? 'request failed'
      const retryAfter = body?.error?.retry_after ?? null
      throw new ApiError(response.status, code, message, retryAfter)
    },
  } satisfies FetchOptions)
}
