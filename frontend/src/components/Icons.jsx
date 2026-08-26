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
