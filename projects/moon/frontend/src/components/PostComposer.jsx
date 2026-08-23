import { useState } from 'react'
import { FiImage, FiSmile, FiMapPin } from 'react-icons/fi'

export default function PostComposer({ onPost }) {
  const [content, setContent] = useState('')
  const [isSubmitting, setIsSubmitting] = useState(false)

  const handleSubmit = async () => {
    if (!content.trim() || isSubmitting) return
    
    setIsSubmitting(true)
    try {
      await onPost(content)
      setContent('')
    } catch (error) {
      console.error('Error al publicar:', error)
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <div className="border-b border-moon-800 p-4">
      <div className="flex gap-3">
        <div className="w-12 h-12 rounded-full bg-moon-700 flex items-center justify-center font-bold flex-shrink-0">
          U
        </div>
        <div className="flex-1">
          <textarea
            value={content}
            onChange={(e) => setContent(e.target.value)}
            placeholder="¿Qué está pasando?"
            className="w-full bg-transparent border-none outline-none resize-none text-xl placeholder-moon-400 min-h-[120px]"
            maxLength={5000}
          />
          
          <div className="flex items-center justify-between border-t border-moon-800 pt-3 mt-2">
            <div className="flex gap-4 text-moon-400">
              <button className="hover:text-moon-300 transition-colors p-2 rounded-full hover:bg-moon-800">
                <FiImage className="text-xl" />
              </button>
              <button className="hover:text-moon-300 transition-colors p-2 rounded-full hover:bg-moon-800">
                <FiSmile className="text-xl" />
              </button>
              <button className="hover:text-moon-300 transition-colors p-2 rounded-full hover:bg-moon-800">
                <FiMapPin className="text-xl" />
              </button>
            </div>
            
            <div className="flex items-center gap-3">
              {content.length > 0 && (
                <span className="text-sm text-moon-400">
                  {content.length}/5000
                </span>
              )}
              <button
                onClick={handleSubmit}
                disabled={!content.trim() || isSubmitting}
                className="bg-white text-moon-900 font-bold px-6 py-2 rounded-full hover:bg-moon-200 disabled:opacity-50 disabled:cursor-not-allowed transition-all"
              >
                {isSubmitting ? 'Publicando...' : 'Publicar'}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
