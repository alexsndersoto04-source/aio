import { useState, useEffect } from 'react'
import { useParams } from 'react-router-dom'
import axios from 'axios'
import PostCard from '../components/PostCard'

const API_URL = import.meta.env.VITE_API_URL || 'http://localhost:3000/api'

export default function Profile() {
  const { id } = useParams()
  const [profile, setProfile] = useState(null)
  const [posts, setPosts] = useState([])
  const [isFollowing, setIsFollowing] = useState(false)
  const [loading, setLoading] = useState(true)
  const [activeTab, setActiveTab] = useState('posts')

  useEffect(() => {
    loadProfile()
  }, [id])

  const loadProfile = async () => {
    try {
      const [profileRes, postsRes] = await Promise.all([
        axios.get(`${API_URL}/users/${id}`),
        axios.get(`${API_URL}/users/${id}/posts`)
      ])
      setProfile(profileRes.data.user)
      setIsFollowing(profileRes.data.is_following)
      setPosts(postsRes.data)
    } catch (error) {
      console.error('Error al cargar perfil:', error)
    } finally {
      setLoading(false)
    }
  }

  const handleFollow = async () => {
    try {
      if (isFollowing) {
        await axios.post(`${API_URL}/unfollow/${id}`)
        setIsFollowing(false)
        setProfile({
          ...profile,
          followers_count: profile.followers_count - 1
        })
      } else {
        await axios.post(`${API_URL}/follow/${id}`)
        setIsFollowing(true)
        setProfile({
          ...profile,
          followers_count: profile.followers_count + 1
        })
      }
    } catch (error) {
      console.error('Error al seguir:', error)
    }
  }

  const handleLike = async (postId) => {
    try {
      await axios.post(`${API_URL}/posts/${postId}/like`)
    } catch (error) {
      console.error('Error al dar like:', error)
    }
  }

  if (loading) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="text-moon-400">Cargando...</div>
      </div>
    )
  }

  if (!profile) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="text-moon-400">Usuario no encontrado</div>
      </div>
    )
  }

  return (
    <div className="min-h-screen">
      {/* Header */}
      <header className="sticky top-0 bg-moon-900/80 backdrop-blur-md border-b border-moon-800 px-4 py-3 z-10">
        <div className="flex items-center gap-4">
          <h1 className="text-xl font-bold">{profile.display_name || profile.username}</h1>
          <span className="text-moon-400 text-sm">{posts.length} posts</span>
        </div>
      </header>

      {/* Banner */}
      <div className="h-48 bg-gradient-to-br from-moon-700 to-moon-800">
        {profile.banner_url && (
          <img src={profile.banner_url} alt="Banner" className="w-full h-full object-cover" />
        )}
      </div>

      {/* Profile Info */}
      <div className="px-4 pb-4 border-b border-moon-800">
        {/* Avatar */}
        <div className="flex justify-between items-start -mt-16 mb-4">
          <div className="w-32 h-32 rounded-full bg-moon-700 border-4 border-moon-900 flex items-center justify-center text-4xl font-bold">
            {profile.username?.[0]?.toUpperCase()}
          </div>
          <button
            onClick={handleFollow}
            className={`mt-20 px-6 py-2 rounded-full font-bold ${
              isFollowing
                ? 'bg-transparent border border-moon-600 text-moon-300 hover:bg-red-500/10 hover:border-red-500 hover:text-red-500'
                : 'bg-white text-moon-900 hover:bg-moon-200'
            }`}
          >
            {isFollowing ? 'Siguiendo' : 'Seguir'}
          </button>
        </div>

        {/* Name & Bio */}
        <div className="mb-4">
          <h2 className="text-2xl font-bold flex items-center gap-2">
            {profile.display_name || profile.username}
            {profile.is_verified && <span className="text-moon-400">✓</span>}
          </h2>
          <p className="text-moon-400">@{profile.username}</p>
          {profile.bio && (
            <p className="mt-3 text-moon-200">{profile.bio}</p>
          )}
        </div>

        {/* Stats */}
        <div className="flex gap-6 text-sm">
          <div>
            <span className="font-bold">{profile.following_count}</span>
            <span className="text-moon-400 ml-1">Siguiendo</span>
          </div>
          <div>
            <span className="font-bold">{profile.followers_count}</span>
            <span className="text-moon-400 ml-1">Seguidores</span>
          </div>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex border-b border-moon-800">
        <button
          onClick={() => setActiveTab('posts')}
          className={`flex-1 py-4 font-medium ${
            activeTab === 'posts'
              ? 'border-b-2 border-white text-white'
              : 'text-moon-400 hover:bg-moon-800'
          }`}
        >
          Posts
        </button>
        <button
          onClick={() => setActiveTab('replies')}
          className={`flex-1 py-4 font-medium ${
            activeTab === 'replies'
              ? 'border-b-2 border-white text-white'
              : 'text-moon-400 hover:bg-moon-800'
          }`}
        >
          Respuestas
        </button>
        <button
          onClick={() => setActiveTab('likes')}
          className={`flex-1 py-4 font-medium ${
            activeTab === 'likes'
              ? 'border-b-2 border-white text-white'
              : 'text-moon-400 hover:bg-moon-800'
          }`}
        >
          Me gusta
        </button>
      </div>

      {/* Posts */}
      <div>
        {posts.length === 0 ? (
          <div className="p-8 text-center text-moon-400">
            No hay posts todavía
          </div>
        ) : (
          posts.map((post) => (
            <PostCard key={post.id} post={post} onLike={handleLike} />
          ))
        )}
      </div>
    </div>
  )
}
