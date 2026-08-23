import { useState } from 'react'
import { Link, useNavigate } from 'react-router-dom'
import { useAuth } from '../context/AuthContext'
import { FiMoon } from 'react-icons/fi'

export default function Register() {
  const [username, setUsername] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const { register } = useAuth()
  const navigate = useNavigate()

  const handleSubmit = async (e) => {
    e.preventDefault()
    setError('')

    if (username.length < 3) {
      setError('El usuario debe tener al menos 3 caracteres')
      return
    }

    if (password.length < 8) {
      setError('La contraseña debe tener al menos 8 caracteres')
      return
    }

    setLoading(true)

    try {
      await register(username, email, password)
      navigate('/')
    } catch (err) {
      setError(err.response?.data?.error || 'Error al registrar. Intenta de nuevo.')
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
          <p className="text-moon-400">Crea tu cuenta</p>
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
              Usuario
            </label>
            <input
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value.toLowerCase())}
              className="w-full bg-moon-700 border border-moon-600 rounded-lg px-4 py-3 focus:outline-none focus:border-moon-400 transition-colors"
              placeholder="@usuario"
              required
            />
          </div>

          <div className="mb-6">
            <label className="block text-sm font-medium text-moon-300 mb-2">
              Email
            </label>
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              className="w-full bg-moon-700 border border-moon-600 rounded-lg px-4 py-3 focus:outline-none focus:border-moon-400 transition-colors"
              placeholder="tu@email.com"
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
              placeholder="Mínimo 8 caracteres"
              required
            />
          </div>

          <button
            type="submit"
            disabled={loading}
            className="w-full bg-white text-moon-900 font-bold py-3 rounded-full hover:bg-moon-200 disabled:opacity-50 disabled:cursor-not-allowed transition-all"
          >
            {loading ? 'Creando cuenta...' : 'Crear cuenta'}
          </button>
        </form>

        {/* Login Link */}
        <div className="text-center mt-6">
          <p className="text-moon-400">
            ¿Ya tienes cuenta?{' '}
            <Link to="/login" className="text-moon-300 hover:underline font-medium">
              Inicia sesión
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
