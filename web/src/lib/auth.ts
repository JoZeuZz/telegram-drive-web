/**
 * Auth-specific API functions.
 *
 * Re-exports the authentication subset from api.ts so callers
 * can import directly from here when only auth is needed.
 */

export {
  // app auth
  login,
  logout,
  authStatus,
  // telegram auth
  telegramConnect,
  telegramStatus,
  telegramRequestCode,
  telegramSignIn,
  telegramCheckPassword,
  telegramLogout,
  // types
  type AuthStatusResponse,
  type TelegramStatusResponse,
  type AuthResult,
} from "./api";
