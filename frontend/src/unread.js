// Moon — Contadores de no leídos (estado global simple)

import { useEffect, useState } from 'react';

const state = { notifications: 0, messages: 0 };
const listeners = new Set();

function emit() {
  for (const fn of listeners) fn({ ...state });
}

export function setUnread(partial) {
  Object.assign(state, partial);
  emit();
}

export function bump(kind) {
  if (kind === 'notification') state.notifications += 1;
  if (kind === 'message') state.messages += 1;
  emit();
}

export function useUnread() {
  const [value, setValue] = useState({ ...state });
  useEffect(() => {
    const fn = () => setValue({ ...state });
    listeners.add(fn);
    return () => listeners.delete(fn);
  }, []);
  return value;
}
