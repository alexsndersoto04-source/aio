import { useState } from 'react'
import { Link } from 'react-router-dom'
import { formatDistanceToNow } from 'date-fns'
import { es } from 'date-fns/locale'
import { FiHeart, FiMessageCircle, FiRepeat, FiShare, FiMoreHorizontal } from 'react-icons/fi'

export default function PostCard({ post, onLike, onComment }) {
  const [showActions, setShowActions] = useState(false)
  const [isLiked, setIsLiked] = useState(post.is_liked || false)
  const [likesCount, setLikesCount] = useState(post.likes_count || 0)

  const handleLike = async () => {
    try {
      if (isLiked) {
        setLikesCount(likesCount - 1)
        setIsLiked(false)
      } else {
        setLikesCount(likesCount + 1)
        setIsLiked(true)
      }
      await onLike(post.id)
    } catch (error) {
      // Revertir en caso de error
      setIsLiked(!isLiked)
      setLikesCount(isLiked ? likesCount + 1 : likesCount - 1)
    }
  }

  const timeAgo = formatDistanceToNow(new Date(post.created_at), {
    addSuffix: true,
    locale: es
  })

  return (
    <article className="border-b border-moon-800 p-4 hover:bg-moon-800/50 transition-colors cursor-pointer">
      <div className="flex gap-3">
        {/* Avatar */}
        <Link to={`/profile/${post.user_id}`}>
          <div className="w-12 h-12 rounded-full bg-moon-700 flex items-center justify-center font-bold flex-shrink-0 hover:opacity-80 transition-opacity">
            {post.username?.[0]?.toUpperCase() || 'U'}
          </div>
        </Link>

        {/* Content */}
        <div className="flex-1 min-w-0">
          {/* Header */}
          <div className="flex items-start justify-between mb-1">
            <div className="flex items-center gap-2 flex-wrap">
              <Link to={`/profile/${post.user_id}`} className="font-bold hover:underline truncate">
                {post.display_name || post.username}
              </Link>
              {post.is_verified && (
                <span className="text-moon-400">✓</span>
              )}
              <span className="text-moon-400 text-sm truncate">@{post.username}</span>
              <span className="text-moon-400">·</span>
              <time className="text-moon-400 text-sm">{timeAgo}</time>
            </div>
            <button className="text-moon-400 hover:text-moon-300 p-1 rounded-full hover:bg-moon-700">
              <FiMoreHorizontal />
            </button>
          </div>

          {/* Post Content */}
          <div className="mb-3">
            <p className="text-base whitespace-pre-wrap break-words">{post.content}</p>
            
            {/* Media */}
            {post.media_urls && post.media_urls.length > 0 && (
              <div className="mt-3 rounded-2xl overflow-hidden border border-moon-800">
                {post.media_urls.map((url, idx) => (
                  <img
                    key={idx}
                    src={url}
                    alt={`Media ${idx + 1}`}
                    className="w-full max-h-96 object-cover"
                  />
                ))}
              </div>
            )}
          </div>

          {/* Actions */}
          <div className="flex items-center justify-between text-moon-400 max-w-md">
            <button
              onClick={(e) => {
                e.stopPropagation()
                handleLike()
              }}
              className={`flex items-center gap-2 group ${
                isLiked ? 'text-red-500' : 'hover:text-red-500'
              }`}
            >
              <div className="p-2 rounded-full group-hover:bg-red-500/10 transition-colors">
                <FiHeart className={isLiked ? 'fill-current' : ''} />
              </div>
              <span className="text-sm">{likesCount > 0 ? likesCount : ''}</span>
            </button>

            <button
              onClick={(e) => {
                e.stopPropagation()
                onComment?.(post.id)
              }}
              className="flex items-center gap-2 group hover:text-moon-300"
            >
              <div className="p-2 rounded-full group-hover:bg-moon-700 transition-colors">
                <FiMessageCircle />
              </div>
              <span className="text-sm">{post.comments_count > 0 ? post.comments_count : ''}</span>
            </button>

            <button className="flex items-center gap-2 group hover:text-moon-300">
              <div className="p-2 rounded-full group-hover:bg-moon-700 transition-colors">
                <FiRepeat />
              </div>
              <span className="text-sm">{post.reposts_count > 0 ? post.reposts_count : ''}</span>
            </button>

            <button className="flex items-center gap-2 group hover:text-moon-300">
              <div className="p-2 rounded-full group-hover:bg-moon-700 transition-colors">
                <FiShare />
              </div>
            </button>
          </div>
        </div>
      </div>
    </article>
  )
}
