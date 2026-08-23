import { createContext, useContext, useState, useEffect } from 'react'
import axios from 'axios'

const API_URL = import.meta.env.VITE_API_URL || 'http://localhost:3000/api'

const AuthContext = createContext(null)

export function AuthProvider({ children }) {
  const [user, setUser] = useState(null)
  const [token, setToken] = useState(localStorage.getItem('moon_token'))
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    if (token) {
      axios.defaults.headers.common['Authorization'] = `Bearer ${token}`
      loadUser()
    } else {
      setLoading(false)
    }
  }, [token])

  const loadUser = async () => {
    try {
      const res = await axios.get(`${API_URL}/auth/me`)
      setUser(res.data)
    } catch (error) {
      logout()
    } finally {
      setLoading(false)
    }
  }

  const login = async (username, password) => {
    const res = await axios.post(`${API_URL}/auth/login`, { username, password })
    localStorage.setItem('moon_token', res.data.token)
    setToken(res.data.token)
    setUser(res.data.user)
    axios.defaults.headers.common['Authorization'] = `Bearer ${res.data.token}`
    return res.data
  }

  const register = async (username, email, password) => {
    const res = await axios.post(`${API_URL}/auth/register`, { username, email, password })
    localStorage.setItem('moon_token', res.data.token)
    setToken(res.data.token)
    setUser(res.data.user)
    axios.defaults.headers.common['Authorization'] = `Bearer ${res.data.token}`
    return res.data
  }

  const logout = () => {
    localStorage.removeItem('moon_token')
    setToken(null)
    setUser(null)
    delete axios.defaults.headers.common['Authorization']
  }

  return (
    <AuthContext.Provider value={{ user, token, loading, login, register, logout }}>
      {children}
    </AuthContext.Provider>
  )
}

export const useAuth = () => {
  const context = useContext(AuthContext)
  if (!context) {
    throw new Error('useAuth must be used within AuthProvider')
  }
  return context
}
