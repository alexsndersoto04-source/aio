import { useState, useEffect } from 'react'
import axios from 'axios'
import { formatDistanceToNow } from 'date-fns'
import { es } from 'date-fns/locale'
import { FiHeart, FiMessageCircle, FiUserPlus, FiAtSign } from 'react-icons/fi'

const API_URL = import.meta.env.VITE_API_URL || 'http://localhost:3000/api'

export default function Notifications() {
  const [notifications, setNotifications] = useState([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    loadNotifications()
  }, [])

  const loadNotifications = async () => {
    try {
      const res = await axios.get(`${API_URL}/notifications`)
      setNotifications(res.data)
    } catch (error) {
      console.error('Error al cargar notificaciones:', error)
    } finally {
      setLoading(false)
    }
  }

  const getIcon = (type) => {
    switch (type) {
      case 'like':
        return <FiHeart className="text-red-500" />
      case 'comment':
        return <FiMessageCircle className="text-moon-400" />
      case 'follow':
        return <FiUserPlus className="text-moon-400" />
      case 'mention':
        return <FiAtSign className="text-moon-400" />
      default:
        return <FiHeart className="text-moon-400" />
    }
  }

  const getMessage = (notif) => {
    switch (notif.type) {
      case 'like':
        return 'le dio me gusta a tu post'
      case 'comment':
        return 'comentó en tu post'
      case 'follow':
        return 'te siguió'
      case 'mention':
        return 'te mencionó en un post'
      default:
        return 'interactuó contigo'
    }
  }

  if (loading) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="text-moon-400">Cargando...</div>
      </div>
    )
  }

  return (
    <div className="min-h-screen">
      {/* Header */}
      <header className="sticky top-0 bg-moon-900/80 backdrop-blur-md border-b border-moon-800 px-4 py-3 z-10">
        <h1 className="text-xl font-bold">Notificaciones</h1>
      </header>

      {/* Notifications List */}
      <div>
        {notifications.length === 0 ? (
          <div className="p-8 text-center">
            <div className="text-moon-400 mb-4">No tienes notificaciones</div>
            <p className="text-moon-500 text-sm">Cuando alguien interactúe contigo, aparecerá aquí</p>
          </div>
        ) : (
          notifications.map((notif) => (
            <div
              key={notif.id}
              className={`flex items-start gap-3 p-4 border-b border-moon-800 ${
                !notif.is_read ? 'bg-moon-800/30' : ''
              }`}
            >
              {/* Icon */}
              <div className="w-10 h-10 rounded-full bg-moon-800 flex items-center justify-center flex-shrink-0">
                {getIcon(notif.type)}
              </div>

              {/* Content */}
              <div className="flex-1 min-w-0">
                <div className="flex items-start gap-2 mb-1">
                  <div className="w-8 h-8 rounded-full bg-moon-700 flex items-center justify-center font-bold text-sm flex-shrink-0">
                    {notif.from_username?.[0]?.toUpperCase()}
                  </div>
                  <div className="flex-1">
                    <p className="text-sm">
                      <span className="font-bold">{notif.from_display_name || notif.from_username}</span>
                      {' '}
                      {getMessage(notif)}
                    </p>
                    <time className="text-xs text-moon-400 mt-1 block">
                      {formatDistanceToNow(new Date(notif.created_at), {
                        addSuffix: true,
                        locale: es
                      })}
                    </time>
                  </div>
                </div>
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  )
}
