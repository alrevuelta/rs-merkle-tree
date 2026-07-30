import re
import subprocess
import shlex
from pathlib import Path
from typing import Iterable, List, Tuple

# store, depth, num_leaves, size_mib
DiskRow = Tuple[str, int, int, float]

def _parse_disk_space(lines: Iterable[str]) -> List[DiskRow]:
    pattern = re.compile(
        r"store\s+(?P<file>\S+)\s+depth\s+(?P<depth>\d+)\s+num_leaves\s+"
        r"(?P<leaves>\d+)\s+size:\s+(?P<size>[\d.]+)\s+MiB",
        re.IGNORECASE,
    )
    rows: List[DiskRow] = []
    for line in lines:
        if (m := pattern.search(line)) is None:
            continue
        file_name: str = m.group("file")
        store = Path(file_name).stem
        depth = int(m.group("depth"))
        leaves = int(m.group("leaves"))
        size_mib = float(m.group("size"))
        rows.append((store, depth, leaves, size_mib))
    return rows


def _disk_table(rows: List[DiskRow]) -> str:
    header = "| Store | Depth | Leaves | Size (MiB) |\n" + "|---|---|---|---|"
    body = "\n".join(
        f"| {store} | {depth} | {leaves} | {size:.2f} |"
        for store, depth, leaves, size in rows
    )
    return f"{header}\n{body}"

# depth, hash, store, time_ms
ProofRow = Tuple[int, str, str, float]

# "get_proof/sqlite_store/depth32_keccak256"
_PROOF_NAME_RE = re.compile(
    r"get_proof/(?P<store>[a-zA-Z0-9]+)_store/" r"depth(?P<depth>\d+)_(?P<hash>[a-zA-Z0-9]+)")
# "time:   [7.8148 ms 7.8307 ms 7.8462 ms]"
_TIME_RE = re.compile(
    r"time:\s*\[\s*(?P<low>[\d.]+)\s+(?P<unit>ns|us|µs|ms|s)\s+"
    r"(?P<mid>[\d.]+)\s+\w+\s+(?P<high>[\d.]+)\s+\w+",
    re.IGNORECASE,
)


def _parse_proof(lines: Iterable[str]) -> List[ProofRow]:
    rows: List[ProofRow] = []
    current: tuple[int, str, str] | None = None  # depth, hash, store

    for line in lines:
        if m := _PROOF_NAME_RE.search(line):
            store = m.group("store")
            depth = int(m.group("depth"))
            hash_alg = m.group("hash")
            current = (depth, hash_alg, store)
            continue

        if current and (time_m := _TIME_RE.search(line)):
            mid_val = float(time_m.group("mid"))
            unit = time_m.group("unit").lower()
            # Convert to milliseconds
            factor = {
                "ns": 1e-6,
                "us": 1e-3,
                "µs": 1e-3,
                "ms": 1.0,
                "s": 1000.0,
            }[unit]
            time_ms = mid_val * factor
            depth, hash_alg, store = current
            rows.append((depth, hash_alg, store, time_ms))
            current = None

    rows.sort(key=lambda row: row[3])  # fastest (lowest ms) first
    return rows


def _proof_table(rows: List[ProofRow]) -> str:
    def _best_unit(time_ms: float) -> tuple[float, str]:
        """Return value and unit chosen to keep number in readable range."""
        if time_ms >= 1000:
            return time_ms / 1000.0, "s"
        if time_ms >= 1:
            return time_ms, "ms"
        if time_ms >= 1e-3:
            return time_ms * 1000.0, "µs"
        return time_ms * 1_000_000.0, "ns"

    header = "| Depth | Hash | Store | Time |\n" + "|---|---|---|---|"
    lines: list[str] = []
    for depth, hash_alg, store, time_ms in rows:
        val, unit = _best_unit(time_ms)
        lines.append(f"| {depth} | {hash_alg} | {store} | {val:.3f} {unit} |")

    body = "\n".join(lines)
    return f"{header}\n{body}"

def _run(cmd: str | list[str], include_stderr: bool = True) -> list[str]:
    cmd_list = shlex.split(cmd) if isinstance(cmd, str) else cmd
    result = subprocess.run(cmd_list, check=True, capture_output=True, text=True)
    output = result.stdout + result.stderr if include_stderr else result.stdout
    return output.splitlines()


def main() -> None:
    # Run disk-space test
    disk_lines = _run("cargo test --release test_disk_space -- --ignored --no-capture")

    print("## Benchmarks\n")

    disk_rows = _parse_disk_space(disk_lines)
    if disk_rows:
        print("### Disk space usage\n")
        print(_disk_table(disk_rows))
        print()

    # The insertions bench is not a Criterion benchmark: it prints the markdown
    # table itself on stdout, so there is nothing to parse here. Progress goes to
    # stderr and is left out.
    # TODO: Report several batch sizes, the harness takes --batch.
    insert_table = "\n".join(
        _run("cargo bench --features all --bench insertions", include_stderr=False)
    ).strip()
    if insert_table:
        print("### `add_leaves` throughput\n")
        print(insert_table)
        print()

    proof_rows = _parse_proof(_run("cargo bench --features all --bench benchmarks"))
    if proof_rows:
        print("### `proof` time\n")
        print(_proof_table(proof_rows))


if __name__ == "__main__":
    main()
