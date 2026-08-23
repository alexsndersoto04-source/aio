import { useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { useAuth } from '../context/AuthContext'
import { FiMoon } from 'react-icons/fi'

export default function Login() {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const { login } = useAuth()
  const navigate = useNavigate()

  const handleSubmit = async (e) => {
    e.preventDefault()
    setError('')
    setLoading(true)

    try {
      await login(username, password)
      navigate('/')
    } catch (err) {
      setError('Credenciales inválidas. Intenta de nuevo.')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-moon-900 px-4">
      <div className="max-w-md w-full">
        {/* Logo */}
        <div className="text-center mb-8">
          <FiMoon className="text-6xl text-moon-300 mx-auto mb-4" />
          <h1 className="text-4xl font-bold mb-2">Moon</h1>
          <p className="text-moon-400">Inicia sesión en tu cuenta</p>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="bg-moon-800 rounded-2xl p-8 shadow-xl">
          {error && (
            <div className="bg-red-500/10 border border-red-500/50 text-red-400 px-4 py-3 rounded-lg mb-4">
              {error}
            </div>
          )}

          <div className="mb-6">
            <label className="block text-sm font-medium text-moon-300 mb-2">
              Usuario o Email
            </label>
            <input
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              className="w-full bg-moon-700 border border-moon-600 rounded-lg px-4 py-3 focus:outline-none focus:border-moon-400 transition-colors"
              placeholder="@usuario"
              required
            />
          </div>

          <div className="mb-6">
            <label className="block text-sm font-medium text-moon-300 mb-2">
              Contraseña
            </label>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className="w-full bg-moon-700 border border-moon-600 rounded-lg px-4 py-3 focus:outline-none focus:border-moon-400 transition-colors"
              placeholder="••••••••"
              required
            />
          </div>

          <button
            type="submit"
            disabled={loading}
            className="w-full bg-white text-moon-900 font-bold py-3 rounded-full hover:bg-moon-200 disabled:opacity-50 disabled:cursor-not-allowed transition-all"
          >
            {loading ? 'Iniciando sesión...' : 'Iniciar sesión'}
          </button>
        </form>

        {/* Register Link */}
        <div className="text-center mt-6">
          <p className="text-moon-400">
            ¿No tienes cuenta?{' '}
            <Link to="/register" className="text-moon-300 hover:underline font-medium">
              Regístrate
            </Link>
          </p>
        </div>

        {/* Footer */}
        <div className="text-center mt-8 text-xs text-moon-500">
          <p>© 2026 Moon. Hecho con Titan</p>
        </div>
      </div>
    </div>
  )
}
