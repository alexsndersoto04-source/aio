import { useState, useEffect } from 'react'
import axios from 'axios'

const API_URL = import.meta.env.VITE_API_URL || 'http://localhost:3000/api'

export default function Messages() {
  const [conversations, setConversations] = useState([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    loadConversations()
  }, [])

  const loadConversations = async () => {
    try {
      const res = await axios.get(`${API_URL}/messages/conversations`)
      setConversations(res.data)
    } catch (error) {
      console.error('Error al cargar mensajes:', error)
    } finally {
      setLoading(false)
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
        <h1 className="text-xl font-bold">Mensajes</h1>
      </header>

      {/* Conversations List */}
      <div>
        {conversations.length === 0 ? (
          <div className="p-8 text-center">
            <div className="text-moon-400 mb-4">No tienes mensajes todavía</div>
            <p className="text-moon-500 text-sm">Cuando envíes o recibas mensajes, aparecerán aquí</p>
          </div>
        ) : (
          conversations.map((conv) => (
            <div
              key={conv.partner_id}
              className="flex items-center gap-3 p-4 hover:bg-moon-800 cursor-pointer border-b border-moon-800"
            >
              {/* Avatar */}
              <div className="w-12 h-12 rounded-full bg-moon-700 flex items-center justify-center font-bold flex-shrink-0">
                {conv.username?.[0]?.toUpperCase()}
              </div>

              {/* Info */}
              <div className="flex-1 min-w-0">
                <div className="flex items-center justify-between mb-1">
                  <div className="font-bold truncate">{conv.display_name || conv.username}</div>
                  <div className="text-xs text-moon-400">
                    {conv.last_message_at && new Date(conv.last_message_at).toLocaleDateString()}
                  </div>
                </div>
                <div className="text-sm text-moon-400 truncate">{conv.last_message}</div>
              </div>

              {/* Unread badge */}
              {conv.unread_count > 0 && (
                <div className="bg-moon-300 text-moon-900 rounded-full w-6 h-6 flex items-center justify-center text-xs font-bold">
                  {conv.unread_count}
                </div>
              )}
            </div>
          ))
        )}
      </div>
    </div>
  )
}
