import { Outlet } from 'react-router-dom'
import Sidebar from './Sidebar'
import MobileNav from './MobileNav'

export default function Layout() {
  return (
    <div className="min-h-screen bg-moon-900">
      <div className="max-w-7xl mx-auto flex">
        {/* Sidebar Desktop */}
        <aside className="hidden md:block w-64 h-screen sticky top-0 border-r border-moon-800">
          <Sidebar />
        </aside>

        {/* Main Content */}
        <main className="flex-1 min-h-screen border-r border-moon-800">
          <Outlet />
        </main>

        {/* Right Sidebar Desktop (opcional para trends, sugerencias) */}
        <aside className="hidden lg:block w-80 p-4">
          <div className="sticky top-4">
            <div className="bg-moon-800 rounded-2xl p-4 mb-4">
              <h3 className="font-bold text-lg mb-3">Tendencias para ti</h3>
              <div className="space-y-3">
                <div className="cursor-pointer hover:bg-moon-700 -mx-2 px-2 py-2 rounded-lg">
                  <div className="text-xs text-moon-400">Tendencia en Tecnología</div>
                  <div className="font-bold">#TitanLang</div>
                  <div className="text-xs text-moon-400">2.4K posts</div>
                </div>
                <div className="cursor-pointer hover:bg-moon-700 -mx-2 px-2 py-2 rounded-lg">
                  <div className="text-xs text-moon-400">Tendencia</div>
                  <div className="font-bold">#OpenSource</div>
                  <div className="text-xs text-moon-400">1.8K posts</div>
                </div>
                <div className="cursor-pointer hover:bg-moon-700 -mx-2 px-2 py-2 rounded-lg">
                  <div className="text-xs text-moon-400">Tendencia</div>
                  <div className="font-bold">#RustLang</div>
                  <div className="text-xs text-moon-400">1.2K posts</div>
                </div>
              </div>
            </div>

            <div className="bg-moon-800 rounded-2xl p-4">
              <h3 className="font-bold text-lg mb-3">A quién seguir</h3>
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <div className="w-10 h-10 rounded-full bg-moon-700 flex items-center justify-center font-bold">
                      A
                    </div>
                    <div>
                      <div className="font-bold text-sm">Alex Soto</div>
                      <div className="text-xs text-moon-400">@alex</div>
                    </div>
                  </div>
                  <button className="bg-white text-moon-900 font-bold px-4 py-1.5 rounded-full text-sm hover:bg-moon-200">
                    Seguir
                  </button>
                </div>
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <div className="w-10 h-10 rounded-full bg-moon-700 flex items-center justify-center font-bold">
                      M
                    </div>
                    <div>
                      <div className="font-bold text-sm">Moon Team</div>
                      <div className="text-xs text-moon-400">@moon</div>
                    </div>
                  </div>
                  <button className="bg-white text-moon-900 font-bold px-4 py-1.5 rounded-full text-sm hover:bg-moon-200">
                    Seguir
                  </button>
                </div>
              </div>
            </div>

            <div className="text-xs text-moon-400 mt-4 px-2">
              <p>© 2026 Moon. Hecho con Titan</p>
            </div>
          </div>
        </aside>
      </div>

      {/* Mobile Bottom Navigation */}
      <MobileNav />
    </div>
  )
}
