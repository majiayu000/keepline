/**
 * AuthLogin - Login form
 */

import { useState, type FormEvent } from 'react'
import styles from './AuthLogin.module.css'

interface AuthLoginProps {
  onLogin: (username: string, password: string, totpCode?: string) => Promise<void>
  onLocalLogin: () => Promise<void>
  error?: string | null
}

export function AuthLogin({ onLogin, onLocalLogin, error }: AuthLoginProps) {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [totpCode, setTotpCode] = useState('')
  const [showTotp, setShowTotp] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  const [localError, setLocalError] = useState<string | null>(null)

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setLocalError(null)
    setSubmitting(true)
    try {
      await onLogin(username, password, totpCode || undefined)
    } catch (e) {
      const msg = (e as Error).message
      if (msg.includes('TOTP')) {
        setShowTotp(true)
      }
      setLocalError(msg)
    } finally {
      setSubmitting(false)
    }
  }

  const handleLocalLogin = async () => {
    setLocalError(null)
    setSubmitting(true)
    try {
      await onLocalLogin()
    } catch (e) {
      setLocalError((e as Error).message)
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className={styles.container}>
      <div className={styles.card}>
        <h2 className={styles.title}>Terminal Login</h2>
        <form onSubmit={handleSubmit} className={styles.form}>
          <div className={styles.field}>
            <label htmlFor="login-username">Username</label>
            <input
              id="login-username"
              type="text"
              value={username}
              onChange={e => setUsername(e.target.value)}
              required
              autoComplete="username"
              autoFocus
            />
          </div>
          <div className={styles.field}>
            <label htmlFor="login-password">Password</label>
            <input
              id="login-password"
              type="password"
              value={password}
              onChange={e => setPassword(e.target.value)}
              required
              autoComplete="current-password"
            />
          </div>
          {showTotp && (
            <div className={styles.field}>
              <label htmlFor="login-totp">TOTP Code</label>
              <input
                id="login-totp"
                type="text"
                value={totpCode}
                onChange={e => setTotpCode(e.target.value)}
                placeholder="6-digit code"
                maxLength={6}
                autoComplete="one-time-code"
              />
            </div>
          )}
          {(localError || error) && (
            <div className={styles.error}>{localError || error}</div>
          )}
          <button type="submit" className={styles.submit} disabled={submitting}>
            {submitting ? 'Logging in...' : 'Login'}
          </button>
          <div className={styles.divider}>or</div>
          <button type="button" className={styles.localBtn} disabled={submitting} onClick={handleLocalLogin}>
            Local Login (no password)
          </button>
        </form>
      </div>
    </div>
  )
}
