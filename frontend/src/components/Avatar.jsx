const SIZES = { xs: 28, sm: 36, md: 44, lg: 64, xl: 96 };

export default function Avatar({ src, name = '?', size = 'md', ...rest }) {
  const s = SIZES[size] || 40;
  if (src) {
    return <img className="avatar" src={src} alt={name} style={{ width: s, height: s }} {...rest} />;
  }
  return (
    <div
      className="avatar avatar-fallback"
      style={{ width: s, height: s, fontSize: Math.round(s * 0.42) }}
      {...rest}
    >
      {(name || '?')[0].toUpperCase()}
    </div>
  );
}
