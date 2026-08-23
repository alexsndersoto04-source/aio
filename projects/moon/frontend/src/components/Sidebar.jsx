import { NavLink, useNavigate } from 'react-router-dom'
import { useAuth } from '../context/AuthContext'
import { FiHome, FiSearch, FiBell, FiMail, FiUser, FiMoon, FiLogOut } from 'react-icons/fi'

export default function Sidebar() {
  const { user, logout } = useAuth()
  const navigate = useNavigate()

  const handleLogout = () => {
    logout()
    navigate('/login')
  }

  const navItems = [
    { to: '/', icon: FiHome, label: 'Inicio' },
    { to: '/search', icon: FiSearch, label: 'Buscar' },
    { to: '/notifications', icon: FiBell, label: 'Notificaciones' },
    { to: '/messages', icon: FiMail, label: 'Mensajes' },
    { to: `/profile/${user?.id}`, icon: FiUser, label: 'Perfil' },
  ]

  return (
    <div className="flex flex-col h-full p-4">
      {/* Logo */}
      <div className="mb-8">
        <div className="flex items-center gap-3">
          <FiMoon className="text-3xl text-moon-300" />
          <span className="text-2xl font-bold">Moon</span>
        </div>
      </div>

      {/* Navigation */}
      <nav className="flex-1 space-y-2">
        {navItems.map(({ to, icon: Icon, label }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              `flex items-center gap-4 px-4 py-3 rounded-full text-lg hover:bg-moon-800 transition-colors ${
                isActive ? 'font-bold' : ''
              }`
            }
          >
            <Icon className="text-2xl" />
            <span>{label}</span>
          </NavLink>
        ))}

        <button
          onClick={handleLogout}
          className="flex items-center gap-4 px-4 py-3 rounded-full text-lg hover:bg-moon-800 transition-colors w-full"
        >
          <FiLogOut className="text-2xl" />
          <span>Cerrar sesión</span>
        </button>
      </nav>

      {/* Post Button */}
      <button className="w-full bg-white text-moon-900 font-bold py-3 rounded-full hover:bg-moon-200 transition-colors mb-4">
        Publicar
      </button>

      {/* User Info */}
      <div className="flex items-center gap-3 p-3 rounded-full hover:bg-moon-800 cursor-pointer">
        <div className="w-10 h-10 rounded-full bg-moon-700 flex items-center justify-center font-bold">
          {user?.username?.[0]?.toUpperCase() || 'U'}
        </div>
        <div className="flex-1 min-w-0">
          <div className="font-bold text-sm truncate">{user?.display_name || user?.username}</div>
          <div className="text-xs text-moon-400 truncate">@{user?.username}</div>
        </div>
      </div>
    </div>
  )
}
