import sys
import subprocess

def main():
    log_file = "/tmp/build.log"
    try:
        with open(log_file, "r", encoding="utf-8", errors="replace") as f:
            lines = f.readlines()
    except Exception as e:
        print(f"No se pudo leer {log_file}: {e}")
        return

    # Buscar líneas relevantes de error
    error_indices = []
    for idx, line in enumerate(lines):
        lower = line.lower()
        if any(term in lower for term in ["what went wrong", "error:", "compilation error", "cannot find symbol", "failure:"]):
            error_indices.append(idx)

    extracted = []
    if error_indices:
        # Extraer ventana alrededor del primer y último bloque de error
        start = max(0, error_indices[0] - 2)
        end = min(len(lines), error_indices[-1] + 25)
        extracted = lines[start:end]
    else:
        # Si no hay coincidencias obvias, tomar las últimas 80 líneas
        extracted = lines[-80:]

    summary_content = "".join(extracted[:80])
    print("=== EXTRACTED GRADLE ERROR ===")
    print(summary_content)

    summary_file = "/tmp/err_summary.log"
    with open(summary_file, "w", encoding="utf-8") as f:
        f.write("```\n" + summary_content + "\n```\n")

    # Intentar comentar en el PR 20
    try:
        subprocess.run(["gh", "pr", "comment", "20", "--body-file", summary_file], check=False)
    except Exception as e:
        print(f"gh pr comment error: {e}")

if __name__ == "__main__":
    main()
