import { useState } from 'react'

interface AuthFormProps {
  onGuestLogin: () => Promise<void>
  onEmailLogin: (email: string, password: string) => Promise<void>
  onEmailRegister: (email: string, password: string, displayName: string) => Promise<void>
  isLoading: boolean
}

type AuthTab = 'guest' | 'login' | 'register'

export function AuthForm({ onGuestLogin, onEmailLogin, onEmailRegister, isLoading }: AuthFormProps) {
  const [activeTab, setActiveTab] = useState<AuthTab>('guest')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [displayName, setDisplayName] = useState('')
  const [error, setError] = useState<string | null>(null)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)

    try {
      if (activeTab === 'guest') {
        await onGuestLogin()
      } else if (activeTab === 'login') {
        if (!email || !password) {
          setError('Please enter email and password')
          return
        }
        await onEmailLogin(email, password)
      } else if (activeTab === 'register') {
        if (!email || !password || !displayName) {
          setError('Please fill in all fields')
          return
        }
        if (password.length < 6) {
          setError('Password must be at least 6 characters')
          return
        }
        await onEmailRegister(email, password, displayName)
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Authentication failed')
    }
  }

  const tabClass = (tab: AuthTab) =>
    `flex-1 py-2 text-sm font-medium rounded-t-lg transition-colors ${
      activeTab === tab
        ? 'bg-felt-medium text-poker-gold border-b-2 border-poker-gold'
        : 'bg-felt-dark text-gray-400 hover:text-white'
    }`

  return (
    <div className="w-full max-w-md mx-auto">
      {/* Title */}
      <h1 className="text-3xl md:text-4xl font-bold text-poker-gold mb-6 text-center font-poker">
        Texas Hold'em Poker
      </h1>

      {/* Tabs */}
      <div className="flex mb-0 border-b border-gray-700">
        <button
          type="button"
          onClick={() => setActiveTab('guest')}
          className={tabClass('guest')}
          disabled={isLoading}
        >
          Play as Guest
        </button>
        <button
          type="button"
          onClick={() => setActiveTab('login')}
          className={tabClass('login')}
          disabled={isLoading}
        >
          Login
        </button>
        <button
          type="button"
          onClick={() => setActiveTab('register')}
          className={tabClass('register')}
          disabled={isLoading}
        >
          Register
        </button>
      </div>

      {/* Form */}
      <form
        onSubmit={handleSubmit}
        className="bg-felt-medium p-6 rounded-b-lg shadow-lg"
      >
        {activeTab === 'guest' && (
          <div className="text-center text-gray-300 mb-4">
            <p className="mb-2">Play instantly without an account!</p>
            <p className="text-sm text-gray-400">
              Your progress won't be saved. Create an account to keep your chips.
            </p>
          </div>
        )}

        {activeTab === 'login' && (
          <div className="space-y-4">
            <div>
              <label htmlFor="login-email" className="block text-sm text-gray-300 mb-1">
                Email
              </label>
              <input
                id="login-email"
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                className="w-full px-4 py-2 bg-felt-dark border border-gray-600 rounded-lg
                           text-white placeholder-gray-500 focus:outline-none focus:border-poker-gold"
                placeholder="your@email.com"
                disabled={isLoading}
              />
            </div>
            <div>
              <label htmlFor="login-password" className="block text-sm text-gray-300 mb-1">
                Password
              </label>
              <input
                id="login-password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="w-full px-4 py-2 bg-felt-dark border border-gray-600 rounded-lg
                           text-white placeholder-gray-500 focus:outline-none focus:border-poker-gold"
                placeholder="Enter password"
                disabled={isLoading}
              />
            </div>
          </div>
        )}

        {activeTab === 'register' && (
          <div className="space-y-4">
            <div>
              <label htmlFor="register-name" className="block text-sm text-gray-300 mb-1">
                Display Name
              </label>
              <input
                id="register-name"
                type="text"
                value={displayName}
                onChange={(e) => setDisplayName(e.target.value)}
                className="w-full px-4 py-2 bg-felt-dark border border-gray-600 rounded-lg
                           text-white placeholder-gray-500 focus:outline-none focus:border-poker-gold"
                placeholder="Your poker name"
                maxLength={20}
                disabled={isLoading}
              />
            </div>
            <div>
              <label htmlFor="register-email" className="block text-sm text-gray-300 mb-1">
                Email
              </label>
              <input
                id="register-email"
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                className="w-full px-4 py-2 bg-felt-dark border border-gray-600 rounded-lg
                           text-white placeholder-gray-500 focus:outline-none focus:border-poker-gold"
                placeholder="your@email.com"
                disabled={isLoading}
              />
            </div>
            <div>
              <label htmlFor="register-password" className="block text-sm text-gray-300 mb-1">
                Password
              </label>
              <input
                id="register-password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="w-full px-4 py-2 bg-felt-dark border border-gray-600 rounded-lg
                           text-white placeholder-gray-500 focus:outline-none focus:border-poker-gold"
                placeholder="At least 6 characters"
                disabled={isLoading}
              />
            </div>
          </div>
        )}

        {error && (
          <div className="mt-4 p-3 bg-red-900/50 border border-red-700 rounded-lg text-red-300 text-sm">
            {error}
          </div>
        )}

        <button
          type="submit"
          disabled={isLoading}
          className="w-full mt-6 py-3 bg-poker-gold text-black font-bold rounded-lg
                     hover:bg-yellow-400 active:bg-yellow-500 transition-colors
                     disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {isLoading ? (
            <span className="flex items-center justify-center gap-2">
              <span className="w-5 h-5 border-2 border-black border-t-transparent rounded-full animate-spin" />
              Connecting...
            </span>
          ) : activeTab === 'guest' ? (
            'Play Now'
          ) : activeTab === 'login' ? (
            'Login'
          ) : (
            'Create Account'
          )}
        </button>

        {activeTab === 'register' && (
          <p className="mt-4 text-center text-xs text-gray-500">
            By registering, you agree to our Terms of Service
          </p>
        )}
      </form>

      {/* Benefits of registering */}
      {activeTab === 'guest' && (
        <div className="mt-6 p-4 bg-felt-medium/50 rounded-lg">
          <h3 className="text-poker-gold font-medium mb-2">Why create an account?</h3>
          <ul className="text-sm text-gray-300 space-y-1">
            <li className="flex items-center gap-2">
              <span className="text-green-400">+</span>
              Save your chips balance
            </li>
            <li className="flex items-center gap-2">
              <span className="text-green-400">+</span>
              Claim daily rewards
            </li>
            <li className="flex items-center gap-2">
              <span className="text-green-400">+</span>
              Appear on leaderboard
            </li>
            <li className="flex items-center gap-2">
              <span className="text-green-400">+</span>
              Track your statistics
            </li>
          </ul>
        </div>
      )}
    </div>
  )
}
