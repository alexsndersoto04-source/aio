# Notas de sysinfo 0.32.1 (Linux), para std::procfs en Titan

## system.rs
- hostname: gethostname(buf, sysconf(HOST_NAME_MAX)=64) (glibc: uname().nodename); cortar en el primer NUL; String::from_utf8 -> si falla None -> "".
- kernel: uname().release, quitando bytes 0, cada byte como char (Latin-1).
- os_name: /etc/os-release, primera línea que empieza con "NAME=" -> resto sin comillas `"`.
  Si no hay: /etc/lsb-release "DISTRIB_ID=". os_version: "VERSION_ID=" / "DISTRIB_RELEASE=". lines() = separa en \n, quita \r final.
  read_to_string falla con UTF-8 inválido -> pasa al fallback.
- uptime: get_all_utf8_data("/proc/uptime", 50); split('.').next().parse::<u64>() o 0.
- load_average: /proc/loadavg; trim().split(' ').take(3).parse::<f64>().unwrap() (pánico si falla); si no abre -> 0,0,0.
- memoria: read_table("/proc/meminfo", ':'): por línea split(':'), key=1ª parte, value=2ª; value.trim_start().split(' ').next() -> u64 (si falla se ignora la línea).
  Campos MemTotal MemFree MemAvailable Buffers Cached Shmem SReclaimable SwapTotal SwapFree, * 1024 saturando.
  Sin MemAvailable: free+buffers+cached+sreclaimable-shmem (saturando).
  used = total - available; used_swap = swap_total - swap_free (resta u64: en release da la vuelta).
- get_all_utf8_data(file, size): ver utils.rs.

## cpu.rs
- /proc/stat leído con BufReader.split(b'\n'). 1ª línea: si line[..4] != "cpu " -> return (pánico si la línea mide <4). Campos: split(' ') sin vacíos, skip(1), 10 valores con to_u64 (sin comprobar dígitos: x*10 + (c-'0') con u8 que da la vuelta?? c - b'0' con c<'0' = pánico de resta en debug, en release vuelta u8) o 0 si faltan.
- Siguientes: mientras line[..3]=="cpu" (pánico si <3 bytes): nombre=1ª parte (from_utf8_unchecked), 10 valores. vendor/brand de get_vendor_id_and_brand()[i] (i = posición, no el número del nombre) o "".
- CpuValues.set: user -= guest (saturando), nice -= guest_nice (saturando). work = user+nice+system+irq+softirq; total = work+idle+iowait+guest+guest_nice+steal (saturando).
- CpuUsage::new_with_values: percent 0 (así cada CPU recién creada tiene usage 0).
- CpuUsage.set: old=new; new=vals; percent = (neww>oldw ? (neww-oldw) as f32 : 0) / (newt>oldt ? (newt-oldt) as f32 : 1) * 100, tope 100. (f32)
- frecuencia (refresh_kind.frequency()): /sys/devices/system/cpu/cpu{pos}/cpufreq/scaling_cur_freq: trim().split('\n').next().parse::<u64>() / 1000.
  Si no: /proc/cpuinfo, primera línea que empieza "cpu MHz\t" | "BogoMIPS" | "clock\t" | "bogomips per cpu"; split(':').last(), replace("MHz",""), trim, parse::<f64> -> as u64 (satura); si no 0.
- get_vendor_id_and_brand: /proc/cpuinfo split('\n'). Línea que empieza "processor\t": índice = split(':').nth(1).trim().parse::<usize>() (si falla continue).
  Bucle interno: "vendor_id\t" -> vendor=get_value; "model name\t" -> brand; "CPU implementer\t" -> impl=hex; "CPU part\t" -> part=hex;
  si no, si has_all_info() || es "processor\t" -> break (¡esa línea se CONSUME: el siguiente procesador se pierde!).
  get_value = split(':').last().trim(). get_hex_value = split(':').last().trim(); si empieza "0x": u32::from_str_radix(&x[2..],16).unwrap() (pánico si inválido), si no 0.
  has_all_info = (brand && vendor) || (impl && part). convert: si impl y part: vendor=arm_implementer(impl) (o ""), brand=arm_part(impl,part) o brand o "". si no vendor/brand o "".
  HashMap insert (índice repetido: el último gana).
- Tablas ARM en arm_tab.txt (copiadas de cpu.rs; implementer y (implementer, part)).

## common/system.rs
- refresh_specifics(everything): memoria; cpu (refresh_cpus(false, everything) = uso + frecuencia); procesos: inner.refresh_processes_specifics(All, kind) y luego, con remove_dead=false, switch_updated() en todos (¡los procesos muertos NO se quitan de la lista!).
- new_all() = SystemInner::new() + refresh_specifics(everything). El Rust de Titan guarda un System global: la 1ª llamada hace new_all() y después refresh_all() + refresh_cpu_list(everything) (CpusWrapper nuevo: CPUs con uso 0 y uso global calculado contra una lectura toda a cero).
- inner.refresh_processes_specifics: uptime(); refresh_procs(/proc, ...); con All: update_procs_cpu(kind) y cpus.set_need_cpus_update().
- update_procs_cpu: cpus.refresh_if_needed(true, uso) (solo si need_cpus_update); si no hay cpus return; (new, old) = total_time del global; total = old>new ? 1 : new-old; total_f32 = total as f32 / ncpus as f32; max = ncpus as f32 * 100; compute_cpu_usage(proc, total_f32, max) a cada proceso.

## process.rs
- refresh_procs: read_dir("/proc") (orden del directorio). Por cada entrada que sea directorio con nombre = usize: pid; primero se recorre /proc/PID/task
  (cada entrada directorio con nombre != PID y numérico -> tarea con parent_pid=Some(PID), recursivo), se agregan las tareas a `data` ANTES que el proceso, luego el proceso (parent None).
  => Los HILOS cuentan como procesos (clave TID).
- _get_process_data(path, pid, parent): si pid ya está: lee stat (con el fd guardado o reabre); parse; st = starttime/clk_tck; si st == guardado -> update_proc_info (set_time etc.), sigue; si no: proceso nuevo en el mismo pid (reemplaza).
  Si no está: lee path/stat, parse, retrieve_all_new_process_info. Cualquier fallo (stat ilegible, parse None) -> se ignora (el viejo queda).
- parse_stat_file: splitn(2,' ') -> 1ª parte (utf8 o None); resto rsplitn(2, ')') -> data (tras el último ')', utf8 o None), short_exe = antes, sin '(' inicial. str_parts = [pid] + data.split_whitespace().
  Índices: State 1, PPid 2, Flags 8, UserTime 13, SystemTime 14, StartTime 21, VSize 22, RSS 23 (indexar fuera de rango = pánico).
- nombre = short_exe (bytes; procfs usa to_string_lossy).
- memoria (refresh memory): /proc/PID[/task/TID]/statm: split(' '): virt = 1º*page, mem = 2º*page (slice_to_nb sin validar). Si no abre/lee: rss(str_parts[23])*page, vsize.
- set_time(utime=parts[13], stime=parts[14] (u64 o 0)): old=actual, actual=nuevo, updated=true.
- compute_cpu_usage(p, total, max): si old_utime==0 && old_stime==0 -> no cambia; si no cpu = min(((ut-out)sat + (st-ost)sat) as f32 / total * 100, max).  f32.
  Se aplica a TODOS los procesos de la lista (también los muertos que no se actualizaron).
- Rust guarda abierto el fd de stat (FileCounter; sube RLIMIT_NOFILE blando al duro). No observable desde Titan salvo el límite de fds.
- exe/cmd/environ/cwd/root/uid/io se leen pero no se usan en std::procfs (no observables).
- top_processes: (pid, name lossy, cpu, mem) en orden del HashMap (aleatorio por ejecución), sort_by cpu desc (estable), take(max(limit,1)); limit negativo -> 10.

## disk.rs (Disks::new_with_refreshed_list, nuevo en cada llamada)
- removibles: read_dir("/dev/disk/by-id/"): nombres que empiezan con "usb-" -> canonicalize (si falla se omite).
- /proc/mounts (utf8, si falla ""), lines(); line.trim_start(); split_whitespace: spec, file, vfstype ("" si faltan).
  file: replace("\\134","\\"), luego "\\040"->" ", "\\011"->"\t", "\\012"->"\n" (en ese orden).
- se descarta si vfstype ∈ {rootfs sysfs proc devtmpfs cgroup cgroup2 pstore squashfs rpc_pipefs iso9660 tmpfs cifs nfs nfs4}
  o file empieza "/sys" o "/proc" o ("/run" y no "/run/media") o spec empieza "sunrpc".
- statvfs(file): total = f_bsize*f_blocks, avail = f_bsize*f_bavail (saturando). Si falla o total==0 -> se descarta.
- removable = algún canonicalizado == spec (comparación exacta de bytes).
- find_type_for_device_name (canonicalize + /sys/block/../rotational) no se usa en la salida.
- JSON: {available, file_system, mount_point, name, removable, total} (BTreeMap: claves ordenadas); nombres con to_string_lossy.

## network.rs (Networks::new_with_refreshed_list, nuevo en cada llamada)
- read_dir("/sys/class/net/") (si falla: mapa vacío); nombre no UTF-8 -> se salta.
- read(parent/statistics/X): un solo read() de hasta 30 bytes; dígitos iniciales -> u64 (sin tope: ret*10 da la vuelta en release); si no abre/lee 0.
- received=rx_bytes, transmitted=tx_bytes, packets_received=rx_packets, packets_transmitted=tx_packets, errors_in=rx_errors, errors_out=tx_errors.
- serde_json::Map (BTreeMap) -> claves ordenadas.

## std::thread::available_parallelism (rustc 48a229ce, library/std/src/sys/thread/unix.rs)
- quota = cgroups::quota().max(1); sched_getaffinity(0, 128 bytes): si ok count = bits; count = min(count, quota); si count != 0 -> count.
- si no: sysconf(_SC_NPROCESSORS_ONLN) (glibc: /sys/devices/system/cpu/online) -> min(quota).
- quota(): /proc/self/cgroup (read_to_end), fold sobre split('\n'):
  fields = splitn(3, ':'); nth(1): "" -> V2; utf8 y alguna parte de split(',') == "cpu" -> V1; otro/None -> previo.
  si previo Some y versión V2 -> previo. path = fields.last()? (None => acumulador None). Some((path[1..], versión)) (path vacío => pánico).
  Resultado None -> usize::MAX.
- quota_v2(g): p = "/sys/fs/cgroup" + g; si no existe p/cgroup.controllers -> MAX. Mientras p empiece (por componentes) con /sys/fs/cgroup:
  leer p/cpu.max (read_to_string): si abre y lee: lines().next()? y split(' ') limit? period? (cualquier None TERMINA todo, devolviendo lo acumulado);
  parse usize ambos y period>0 -> quota = min(quota, limit/period). p = padre.
- quota_v1(g): montajes: "/sys/fs/cgroup/cpu", "/sys/fs/cgroup/cpu,cpuacct", find_mountpoint (mountinfo: campos split(' '): nth(3)=sub_path, siguiente=mount_point,
  último=opciones de superbloque, penúltimo-1 (nth_back(1))=tipo; tipo=="cgroup" y alguna opción == "cpu"; sub_path sin "/" inicial (si no empieza con "/": None => todo None);
  g debe empezar (por componentes) con sub_path; g recortado).
  Para el primer montaje con mount+g existente: mientras p empiece con mount: limit = trim().parse usize de cpu.cfs_quota_us, period de cpu.cfs_period_us; ambos y period>0 -> min(limit/period). p = padre. break.
