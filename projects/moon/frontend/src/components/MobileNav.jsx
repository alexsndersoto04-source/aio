import { NavLink } from 'react-router-dom'
import { FiHome, FiSearch, FiBell, FiMail, FiUser } from 'react-icons/fi'

export default function MobileNav() {
  const navItems = [
    { to: '/', icon: FiHome },
    { to: '/search', icon: FiSearch },
    { to: '/notifications', icon: FiBell },
    { to: '/messages', icon: FiMail },
    { to: '/profile/me', icon: FiUser },
  ]

  return (
    <nav className="md:hidden fixed bottom-0 left-0 right-0 bg-moon-900 border-t border-moon-800">
      <div className="flex justify-around items-center">
        {navItems.map(({ to, icon: Icon }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              `p-4 transition-colors ${
                isActive ? 'text-white' : 'text-moon-400'
              }`
            }
          >
            <Icon className="text-2xl" />
          </NavLink>
        ))}
      </div>
    </nav>
  )
}
