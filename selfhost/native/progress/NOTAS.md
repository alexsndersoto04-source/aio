# std::progress: notas del código de indicatif 0.17.11 + console 0.15.11

Crates (Cargo.lock): indicatif 0.17.11 (deps console, number_prefix,
portable-atomic, unicode-width 0.2.2, web-time), console 0.15.11 (deps
encode_unicode, libc, once_cell, unicode-width 0.2.2). Sin unicode-segmentation:
`segment(s)` = un elemento por `char`; `measure(s)` = UnicodeWidthStr::width
(unicode-width 0.2.2).

## Titan (crates/titan_stdlib/src/progress_mod.rs)

- Registro global `(runtime, id) -> ProgressBar`, next_id empieza en 1,
  checked_add ("progress handle space exhausted"). Máximo 64 activos por
  runtime: "progress handle quota exceeded (limit 64)".
- Mensajes > 4096 bytes: "progress message exceeds byte limit 4096"
  (set_message y finish, antes de buscar la barra).
- VM: bar_new total<0 -> "total must be nonnegative"; set_position <0 ->
  "position must be nonnegative"; increment <0 -> "delta must be nonnegative".
- IDs desconocidos: no hacen nada.
- bar_new(total): ProgressBar::new(total), estilo
  `{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}`
  progress_chars("=> ").
- spinner_new(): ProgressBar::new_spinner(), enable_steady_tick(100 ms),
  estilo `{spinner:.green} {msg}` tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ") (9 chars; el último
  " " es el final).
- set_message -> bar.set_message; set_position; increment -> inc(delta).
- finish(id, msg): quita del registro; msg vacío -> finish(), si no
  finish_with_message(msg).
- abandon(id): quita y finish_and_clear().

## style.rs

- Estilo por defecto antes de set_style: tick_strings
  "⠁⠁⠉⠙⠚⠒⠂⠂⠒⠲⠴⠤⠄⠄⠤⠠⠠⠤⠦⠖⠒⠐⠐⠒⠓⠋⠉⠈⠈ ", progress_chars "█░",
  tab_width 8 (DEFAULT_TAB_WIDTH).
- current_tick_str: si terminado -> último; si no tick_strings[tick % (n-1)].
- format_bar(fract: f32, width, alt_style): width /= char_width (1);
  fill = fract * width as f32 (f32); entirely_filled = fill as usize;
  head = fill > 0.0 && entirely_filled < width; cur: n = len-2 (aquí 1) ->
  cur_char = 1 (">"); bg = width - filled - head (sat);
  salida: chars[0] × filled, chars[cur], luego alt_style.apply_to(chars[len-1]
  × bg) (alt_style None -> Style::new(), sin códigos).
- format_state: pos = state.pos(); len = state.len().unwrap_or(pos).
  Por cada parte: Placeholder -> buf según key:
  - "bar" -> format_bar(state.fraction(), width.unwrap_or(20), alt_style)
  - "spinner" -> current_tick_str
  - "msg" -> state.message.expanded() (tabs -> 8 espacios)
  - "pos" "{pos}", "len" "{len}"
  - "elapsed_precise" -> FormattedDuration(state.elapsed())
  luego si width Some(w): PaddedStringDisplay{buf, w, align, truncate} y
  style.apply_to(eso) si hay style; si width None: style.apply_to(buf) o buf.
  Literal -> texto. NewLine -> push_line. Al final si cur no vacío push_line.
- push_line: sin wide; si el texto tiene '\n' se parte en varias
  LineType::Bar; si no, una.
- PaddedStringDisplay: cols = measure_text_width(str); excess = cols - w (sat);
  si excess > 0 y !truncate -> str tal cual; si excess>0 y truncate (Left) ->
  str[0..len-excess] (bytes; si no es frontera -> str entero);
  si no: diff = w - cols; Left: str + diff espacios.
- Plantillas ya analizadas (Template::from_str):
  T1 = [Ph spinner style=green] [Lit " ["] [Ph elapsed_precise] [Lit "] ["]
       [Ph bar width=40 style=cyan alt=blue truncate=false align=Left]
       [Lit "] "] [Ph pos] [Lit "/"] [Ph len] [Lit " "] [Ph msg]
  T2 = [Ph spinner style=green] [Lit " "] [Ph msg]

## draw_target.rs

- ProgressDrawTarget::stderr() = Term::buffered_stderr(), 20 Hz.
- is_hidden: !term.is_term(). drawable(force, now): si !is_term -> None;
  si force || rate_limiter.allow(now) -> dibuja, si no nada.
- RateLimiter: interval = 1000/20 = 50 ms, capacity = MAX_BURST 20,
  prev = Instant::now() al crear. allow(now): now < prev -> false;
  elapsed = now-prev; si capacity==0 && elapsed < 50 ms -> false;
  new = elapsed_ms / 50; remainder = elapsed_ns % 50_000_000;
  capacity = min(20, capacity + new - 1); prev = now - remainder; true.
- draw_to_term(term, bar_count):
  - si lines no vacío && move_cursor: (move_cursor es false por defecto)
  - si no: n = bar_count; move_cursor_up(n-1 sat); para i en 0..n:
    clear_line; si i+1 != n move_cursor_down(1); luego move_cursor_up(n-1 sat).
  - term_width = term.width(); alineación Top (sin shift).
  - real_height = 0; por cada línea idx: h = wrapped_height(tw);
    Bar: si real_height + h > term.height() -> break; real_height += h.
    si idx != 0 -> write_line(""); write_str(line);
    si es la última: filler = h*tw - console_width(line); write_str(" "×filler).
  - term.flush(); bar_count = real_height.
- Drawable::clear = state.reset() (lines vacías) y draw.
- wrapped_height = max(ceil(console_width / width as f64), 1).
- console_width = console::measure_text_width.

## progress_bar.rs / state.rs

- ProgressBar::new(len) = with_draw_target(Some(len), stderr()): crea
  AtomicPosition::new() (pos 0, capacity 10, prev 0, start = now), BarState
  (on_finish AndClear por defecto, estilo default_bar, tab_width 8),
  ProgressState::new (tick 0, status InProgress, started = now, message "").
- new_spinner() = with_draw_target(None, stderr()) + set_style(default_spinner
  = "{spinner} {msg}" con tick_strings por defecto). set_style NO dibuja.
- Titan spinner_new: new_spinner(); enable_steady_tick(100ms) (arranca el
  hilo YA, con el estilo por defecto); luego set_style(T2 + tick_chars).
- Ticker (hilo): bucle: si la barra ya no existe -> sale; lock; si
  is_finished -> sale; state.tick(now); unlock; espera 100 ms (o aviso de
  parar -> sale). O sea: el primer tick es inmediato al crear.
- tick(now) [BarState]: tick = tick.saturating_add(1);
  update_estimate_and_draw(now) -> draw(false, now).
- ProgressBar::tick_inner: solo si NO hay ticker -> state.tick(now).
- inc(delta): pos.fetch_add(delta) (u64 envolvente); si pos.allow(now) ->
  tick_inner(now). set_position(p): pos = p; si pos.allow(now) -> tick_inner.
  (Para spinners con ticker: inc/set_position no dibujan.)
- AtomicPosition::allow(now): INTERVAL 1 ms, MAX_BURST 10; elapsed = ns desde
  start; diff = elapsed - prev (sat); si capacity == 0 && diff < 1ms -> false;
  new = diff / 1ms; rem = diff % 1ms; capacity = min(10, capacity + new - 1);
  prev = elapsed - rem; true.
- set_message(msg): message = TabExpandedString(msg) (tabs -> 8 espacios);
  update_estimate_and_draw(now) -> draw(false, now).
- finish() = finish_using_style(now, AndLeave); finish_with_message(m) =
  WithMessage(m); finish_and_clear() = AndClear.
- finish_using_style: status = DoneVisible; AndLeave/WithMessage/AndClear: si
  len Some -> pos = len; WithMessage -> message = m; AndClear -> status =
  DoneHidden. Luego draw(true, now).
- draw(force, now): force |= is_finished; drawable(force, now) o nada;
  width = term.size().1; lines = []; si status != DoneHidden ->
  format_state(state, lines, width); draw_to_term.
- fraction(): len None -> 0; len 0 -> 1; pos 0 -> 0; si no pos as f32 / len as
  f32; clamp(0, 1).
- elapsed() = started.elapsed().
- Drop de BarState: si no terminado -> finish_using_style(on_finish=AndClear).

## Verificado contra el código local (selfhost/fuentes/)

- console: `strip_ansi_codes` (feature ansi-parsing, activada por indicatif),
  `colors_enabled` para STDOUT (TERM != dumb, NO_COLOR, CLICOLOR,
  CLICOLOR_FORCE), `Term::size` (TIOCGWINSZ o 24x80), `clear_line`
  `\r\x1b[2K`, `move_cursor_up/down` solo si n > 0, buffer + un write_all en
  flush (si falla, el buffer se conserva).
- FormattedDuration: `{d}d {h:02}:{m:02}:{s:02}` o `{h:02}:{m:02}:{s:02}`.
- La VM (RuntimeState::drop -> cleanup_runtime) hace finish_and_clear de las
  barras que quedan antes de imprimir el resultado o el error.
- Implementación: native/std_progress.titan. Prueba en terminal:
  `python3 native/progress/pty_run.py 30 100 PROGRAMA` (VM y nativo).
