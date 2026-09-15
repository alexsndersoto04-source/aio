// Moon — Iconos (SVG trazo fino, 1.8px, estilo único)
// ============================================================

const S = (props) => ({
  xmlns: 'http://www.w3.org/2000/svg',
  viewBox: '0 0 24 24',
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.8,
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
  ...props,
});

export const IconHome = (p) => (
  <svg {...S(p)}><path d="M3 10.5 12 3l9 7.5" /><path d="M5 9.5V21h14V9.5" /></svg>
);
export const IconExplore = (p) => (
  <svg {...S(p)}><circle cx="11" cy="11" r="7" /><path d="m20 20-3.8-3.8" /></svg>
);
export const IconBell = (p) => (
  <svg {...S(p)}><path d="M6 9a6 6 0 0 1 12 0c0 5 2 6 2 6H4s2-1 2-6" /><path d="M10 20a2.2 2.2 0 0 0 4 0" /></svg>
);
export const IconMail = (p) => (
  <svg {...S(p)}><rect x="3" y="5" width="18" height="14" rx="3" /><path d="m3 7 9 6 9-6" /></svg>
);
export const IconUser = (p) => (
  <svg {...S(p)}><circle cx="12" cy="8" r="4" /><path d="M4 21c1.5-4 4.5-6 8-6s6.5 2 8 6" /></svg>
);
export const IconUsers = (p) => (
  <svg {...S(p)}><circle cx="9" cy="8" r="3.5" /><path d="M2.5 20c1.2-3.4 3.6-5 6.5-5s5.3 1.6 6.5 5" /><path d="M16 4.6a3.5 3.5 0 0 1 0 6.8" /><path d="M18.5 15.4c1.3 1 2.2 2.4 2.7 4.1" /></svg>
);
export const IconHeart = ({ filled, ...p }) => (
  <svg {...S(p)} fill={filled ? 'currentColor' : 'none'} strokeWidth={filled ? 0 : 1.8}>
    <path d="M12 20.5C6.5 16.5 3 13.2 3 9.6 3 7 5 5 7.4 5c1.8 0 3.4 1 4.6 2.7C13.2 6 14.8 5 16.6 5 19 5 21 7 21 9.6c0 3.6-3.5 6.9-9 10.9Z" />
  </svg>
);
export const IconBookmark = ({ filled, ...p }) => (
  <svg {...S(p)} fill={filled ? 'currentColor' : 'none'} strokeWidth={filled ? 0 : 1.8}>
    <path d="M6 3h12v18l-6-4.5L6 21V3Z" />
  </svg>
);
export const IconComment = (p) => (
  <svg {...S(p)}><path d="M21 11.5a8.5 8.5 0 0 1-8.5 8.5c-1.5 0-3-.4-4.2-1L3 20l1.1-4.8A8.5 8.5 0 1 1 21 11.5Z" /></svg>
);
export const IconSearch = (p) => (
  <svg {...S(p)}><circle cx="11" cy="11" r="7" /><path d="m20 20-3.8-3.8" /></svg>
);
export const IconSettings = (p) => (
  <svg {...S(p)}><circle cx="12" cy="12" r="3" /><path d="M19 12a7 7 0 0 0-.15-1.4l2-1.5-2-3.4-2.3 1a7 7 0 0 0-2.4-1.4L13.8 2h-3.6l-.4 2.7a7 7 0 0 0-2.4 1.4l-2.3-1-2 3.4 2 1.5A7 7 0 0 0 5 12c0 .48.05.95.15 1.4l-2 1.5 2 3.4 2.3-1a7 7 0 0 0 2.4 1.4l.4 2.7h3.6l.4-2.7a7 7 0 0 0 2.4-1.4l2.3 1 2-3.4-2-1.5c.1-.45.15-.92.15-1.4Z" /></svg>
);
export const IconSend = (p) => (
  <svg {...S(p)}><path d="m22 2-7 20-4-9-9-4 20-7Z" /></svg>
);
export const IconLogout = (p) => (
  <svg {...S(p)}><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" /><path d="m16 17 5-5-5-5" /><path d="M21 12H9" /></svg>
);
export const IconVerified = (p) => (
  <svg {...S(p)} fill="currentColor" stroke="none">
    <path d="M12 2.5 14.6 5l3.4-.4.9 3.3 2.9 1.9-1.3 3.2 1.3 3.2-2.9 1.9-.9 3.3-3.4-.4L12 23.5 9.4 21l-3.4.4-.9-3.3-2.9-1.9 1.3-3.2L2.2 9.8l2.9-1.9.9-3.3 3.4.4L12 2.5Zm-1.2 13.6 5.4-5.5-1.4-1.4-4 4.1-1.9-1.9-1.4 1.4 3.3 3.3Z" />
  </svg>
);
export const IconShield = (p) => (
  <svg {...S(p)}><path d="M12 2 4 5.5V11c0 5 3.4 9.4 8 11 4.6-1.6 8-6 8-11V5.5L12 2Z" /><path d="m8.8 11.8 2.2 2.2 4.2-4.4" /></svg>
);
export const IconMore = (p) => (
  <svg {...S(p)}><circle cx="12" cy="5.5" r="1.4" fill="currentColor" stroke="none" /><circle cx="12" cy="12" r="1.4" fill="currentColor" stroke="none" /><circle cx="12" cy="18.5" r="1.4" fill="currentColor" stroke="none" /></svg>
);
export const IconEdit = (p) => (
  <svg {...S(p)}><path d="M12 20h9" /><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5Z" /></svg>
);
export const IconTrash = (p) => (
  <svg {...S(p)}><path d="M3 6h18" /><path d="M8 6V4h8v2" /><path d="M19 6 18 21H6L5 6" /><path d="M10 10v7M14 10v7" /></svg>
);
export const IconPlus = (p) => (
  <svg {...S(p)}><path d="M12 5v14M5 12h14" /></svg>
);
export const IconImage = (p) => (
  <svg {...S(p)}><rect x="3" y="4" width="18" height="16" rx="3" /><circle cx="8.5" cy="9.5" r="1.5" /><path d="m21 16-4.5-4.5L6 21" /></svg>
);
export const IconMoon = (p) => (
  <svg {...S(p)}><path d="M20 14.5A8.5 8.5 0 0 1 9.5 4 8.5 8.5 0 1 0 20 14.5Z" /></svg>
);
export const IconArrow = (p) => (
  <svg {...S(p)}><path d="M5 12h14M13 6l6 6-6 6" /></svg>
);
export const IconX = (p) => (
  <svg {...S(p)}><path d="m6 6 12 12M18 6 6 18" /></svg>
);
export const IconReport = (p) => (
  <svg {...S(p)}><path d="M12 3 2.5 20h19L12 3Z" /><path d="M12 10v4" /><circle cx="12" cy="17" r=".6" fill="currentColor" stroke="none" /></svg>
);
export const IconLock = (p) => (
  <svg {...S(p)}><rect x="5" y="10.5" width="14" height="10" rx="2.5" /><path d="M8 10.5V7.5a4 4 0 0 1 8 0v3" /></svg>
);

// ---- Iconos del sistema «Órbita» (avisos, temas, panel) ----
export const IconCheck = (p) => (
  <svg {...S(p)}><circle cx="12" cy="12" r="9" /><path d="m8.2 12.4 2.6 2.6 5-5.4" /></svg>
);
export const IconAlert = (p) => (
  <svg {...S(p)}><circle cx="12" cy="12" r="9" /><path d="M12 7.5v5.5" /><circle cx="12" cy="16.4" r=".8" fill="currentColor" stroke="none" /></svg>
);
export const IconInfo = (p) => (
  <svg {...S(p)}><circle cx="12" cy="12" r="9" /><path d="M12 11v5.5" /><circle cx="12" cy="7.8" r=".8" fill="currentColor" stroke="none" /></svg>
);
export const IconWarning = (p) => (
  <svg {...S(p)}><path d="M12 3.5 2.8 20h18.4L12 3.5Z" /><path d="M12 10v4.2" /><circle cx="12" cy="17.2" r=".8" fill="currentColor" stroke="none" /></svg>
);
export const IconSun = (p) => (
  <svg {...S(p)}><circle cx="12" cy="12" r="4.2" /><path d="M12 2.6v2.2M12 19.2v2.2M2.6 12h2.2M19.2 12h2.2M5.4 5.4l1.6 1.6M17 17l1.6 1.6M18.6 5.4 17 7M7 17l-1.6 1.6" /></svg>
);
export const IconTrend = (p) => (
  <svg {...S(p)}><path d="M3 17.5 9 11l4 4 8-8.5" /><path d="M15 6.5h6v6" /></svg>
);
export const IconSpark = (p) => (
  <svg {...S(p)}><path d="M12 3.2 13.7 9l5.8 1.7-5.8 1.7L12 18.2 10.3 12.4 4.5 10.7 10.3 9 12 3.2Z" /><path d="M19 17.5l.7 2.1 2.1.7-2.1.7-.7 2.1-.7-2.1-2.1-.7 2.1-.7.7-2.1Z" /></svg>
);
export const IconGlobe = (p) => (
  <svg {...S(p)}><circle cx="12" cy="12" r="9" /><path d="M3.5 9.5h17M3.5 14.5h17" /><path d="M12 3c2.6 3 2.6 15 0 18M12 3c-2.6 3-2.6 15 0 18" /></svg>
);
export const IconCalendar = (p) => (
  <svg {...S(p)}><rect x="3.5" y="5" width="17" height="16" rx="3" /><path d="M8 3v4M16 3v4M3.5 10h17" /></svg>
);
export const IconLink = (p) => (
  <svg {...S(p)}><path d="M10 13.5a3.5 3.5 0 0 0 5 0l3-3a3.5 3.5 0 0 0-5-5l-1.2 1.2" /><path d="M14 10.5a3.5 3.5 0 0 0-5 0l-3 3a3.5 3.5 0 0 0 5 5l1.2-1.2" /></svg>
);
export const IconBan = (p) => (
  <svg {...S(p)}><circle cx="12" cy="12" r="9" /><path d="m5.6 5.6 12.8 12.8" /></svg>
);
export const IconRefresh = (p) => (
  <svg {...S(p)}><path d="M20 11.5A8 8 0 0 0 6.3 6.3L4 8.5" /><path d="M4 4v4.5h4.5" /><path d="M4 12.5A8 8 0 0 0 17.7 17.7L20 15.5" /><path d="M20 20v-4.5h-4.5" /></svg>
);
export const IconChevronLeft = (p) => (
  <svg {...S(p)}><path d="m14.5 6-6 6 6 6" /></svg>
);
export const IconGrid = (p) => (
  <svg {...S(p)}><rect x="3.5" y="3.5" width="7" height="7" rx="2" /><rect x="13.5" y="3.5" width="7" height="7" rx="2" /><rect x="3.5" y="13.5" width="7" height="7" rx="2" /><rect x="13.5" y="13.5" width="7" height="7" rx="2" /></svg>
);
export const IconAt = (p) => (
  <svg {...S(p)}><circle cx="12" cy="12" r="3.6" /><path d="M15.6 8.4v4.9a3.5 3.5 0 0 0 7 .5v-1.8a10.6 10.6 0 1 0-4.4 8.5" /></svg>
);
export const IconMapPin = (p) => (
  <svg {...S(p)}><path d="M12 21s7-5.6 7-11a7 7 0 1 0-14 0c0 5.4 7 11 7 11Z" /><circle cx="12" cy="10" r="2.6" /></svg>
);
export const IconEye = (p) => (
  <svg {...S(p)}><path d="M2.5 12S6 6 12 6s9.5 6 9.5 6-3.5 6-9.5 6-9.5-6-9.5-6Z" /><circle cx="12" cy="12" r="2.8" /></svg>
);
export const IconCopy = (p) => (
  <svg {...S(p)}><rect x="9" y="9" width="11" height="11" rx="2.5" /><path d="M15 6.5V5.5A2.5 2.5 0 0 0 12.5 3h-6A3.5 3.5 0 0 0 3 6.5v6A2.5 2.5 0 0 0 5.5 15h1" /></svg>
);
export const IconCamera = (p) => (
  <svg {...S(p)}><path d="M4 8.5h3l1.5-2.5h7L17 8.5h3a1.5 1.5 0 0 1 1.5 1.5v8A1.5 1.5 0 0 1 20 19.5H4A1.5 1.5 0 0 1 2.5 18v-8A1.5 1.5 0 0 1 4 8.5Z" /><circle cx="12" cy="13.5" r="3.5" /></svg>
);
export const IconLayers = (p) => (
  <svg {...S(p)}><path d="m12 3 9 5-9 5-9-5 9-5Z" /><path d="m3.5 12.5 8.5 4.7 8.5-4.7" /><path d="m3.5 16.5 8.5 4.7 8.5-4.7" /></svg>
);

// --- Chat y notas de voz ---
export const IconResponder = (p) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" {...p}>
    <path d="M9 17l-5-5 5-5" />
    <path d="M4 12h9a6 6 0 0 1 6 6v2" />
  </svg>
);

export const IconChat = (p) => (
  <svg {...S(p)}><path d="M21 12a8 8 0 0 1-8 8H8l-5 3 1.4-4.4A8 8 0 1 1 21 12Z" /><path d="M8.5 12h.01M12 12h.01M15.5 12h.01" /></svg>
);
export const IconMic = (p) => (
  <svg {...S(p)}><rect x="9" y="3" width="6" height="11" rx="3" /><path d="M5 11a7 7 0 0 0 14 0" /><path d="M12 18v3" /><path d="M8.5 21h7" /></svg>
);
export const IconPlay = (p) => (
  <svg {...S(p)}><path d="M7 4.5v15l12-7.5-12-7.5Z" /></svg>
);
export const IconPause = (p) => (
  <svg {...S(p)}><path d="M9 5v14M15 5v14" /></svg>
);
