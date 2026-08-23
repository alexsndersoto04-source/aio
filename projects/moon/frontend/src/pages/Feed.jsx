import { useState, useEffect } from 'react'
import axios from 'axios'
import PostComposer from '../components/PostComposer'
import PostCard from '../components/PostCard'

const API_URL = import.meta.env.VITE_API_URL || 'http://localhost:3000/api'

export default function Feed() {
  const [posts, setPosts] = useState([])
  const [loading, setLoading] = useState(true)
  const [page, setPage] = useState(1)

  useEffect(() => {
    loadFeed()
  }, [page])

  const loadFeed = async () => {
    try {
      const res = await axios.get(`${API_URL}/feed?page=${page}`)
      setPosts(res.data)
    } catch (error) {
      console.error('Error al cargar feed:', error)
    } finally {
      setLoading(false)
    }
  }

  const handleCreatePost = async (content) => {
    const res = await axios.post(`${API_URL}/posts`, { content })
    setPosts([res.data, ...posts])
  }

  const handleLike = async (postId) => {
    try {
      await axios.post(`${API_URL}/posts/${postId}/like`)
    } catch (error) {
      console.error('Error al dar like:', error)
    }
  }

  return (
    <div className="min-h-screen">
      {/* Header */}
      <header className="sticky top-0 bg-moon-900/80 backdrop-blur-md border-b border-moon-800 px-4 py-3 z-10">
        <h1 className="text-xl font-bold">Inicio</h1>
      </header>

      {/* Composer */}
      <PostComposer onPost={handleCreatePost} />

      {/* Posts */}
      <div>
        {loading ? (
          <div className="p-8 text-center text-moon-400">Cargando...</div>
        ) : posts.length === 0 ? (
          <div className="p-8 text-center">
            <div className="text-moon-400 mb-4">No hay posts todavía</div>
            <p className="text-moon-500">Sigue a personas para ver sus posts en tu inicio</p>
          </div>
        ) : (
          posts.map((post) => (
            <PostCard key={post.id} post={post} onLike={handleLike} />
          ))
        )}
      </div>

      {/* Load More */}
      {posts.length > 0 && (
        <div className="p-4 text-center">
          <button
            onClick={() => setPage(page + 1)}
            className="text-moon-400 hover:text-moon-300 font-medium"
          >
            Cargar más
          </button>
        </div>
      )}
    </div>
  )
}
