#!/usr/bin/env bash
set -euo pipefail

artifact_dir=$(realpath -- "${1:-${CARGO_BUILD_ARTIFACT_DIR:-target/artifacts}}")
temporary_dir=$(mktemp -d)
trap 'rm -rf -- "$temporary_dir"' EXIT
bundle="$temporary_dir/relocated bundle"
mkdir -p "$bundle"
cp -- "$artifact_dir/sima-init" "$artifact_dir/simactl" \
    "$artifact_dir/libsima_proto.so" "$artifact_dir/libsima_runtime.so" "$bundle/"

for executable in sima-init simactl; do
    dynamic_section=$(LC_ALL=C readelf -d "$bundle/$executable")
    [[ "$dynamic_section" == *'Shared library: [libsima_proto.so]'* ]]
    [[ "$dynamic_section" == *'Shared library: [libsima_runtime.so]'* ]]
    [[ "$dynamic_section" != *'Shared library: [libstd-'* ]]
    [[ "$dynamic_section" == *'$ORIGIN'* ]]
done

dynamic_section=$(LC_ALL=C readelf -d "$bundle/libsima_proto.so")
[[ "$dynamic_section" == *'Shared library: [libsima_runtime.so]'* ]]
[[ "$dynamic_section" != *'Shared library: [libstd-'* ]]
dynamic_section=$(LC_ALL=C readelf -d "$bundle/libsima_runtime.so")
[[ "$dynamic_section" == *'Library soname: [libsima_runtime.so]'* ]]

env -u LD_LIBRARY_PATH -u LD_PRELOAD "$bundle/simactl" --help >/dev/null
mv "$bundle/libsima_proto.so" "$temporary_dir/libsima_proto.so"
if env -u LD_LIBRARY_PATH -u LD_PRELOAD "$bundle/simactl" --help \
    >"$temporary_dir/missing-library.log" 2>&1; then
    echo "simactl unexpectedly ran without libsima_proto.so" >&2
    exit 1
fi
[[ "$(cat "$temporary_dir/missing-library.log")" == *'libsima_proto.so'* ]]
mv "$temporary_dir/libsima_proto.so" "$bundle/libsima_proto.so"

mv "$bundle/libsima_runtime.so" "$temporary_dir/libsima_runtime.so"
if env -u LD_LIBRARY_PATH -u LD_PRELOAD "$bundle/simactl" --help \
    >"$temporary_dir/missing-runtime.log" 2>&1; then
    echo "simactl unexpectedly ran without libsima_runtime.so" >&2
    exit 1
fi
[[ "$(cat "$temporary_dir/missing-runtime.log")" == *'libsima_runtime.so'* ]]
mv "$temporary_dir/libsima_runtime.so" "$bundle/libsima_runtime.so"

cat >"$bundle/client.sh" <<'CLIENT'
#!/usr/bin/env bash
set -euo pipefail
[[ "$(readlink /proc/1/exe)" == "$SIMA_SMOKE_BUNDLE/sima-init" ]]
for attempt in {1..100}; do
    if [[ -S /run/sima.sock ]]; then
        break
    fi
    sleep 0.05
done
[[ -S /run/sima.sock ]]
for operation in start stop; do
    response=$("$SIMA_SMOKE_BUNDLE/simactl" "$operation" shared-library-probe)
    [[ "$response" == OK ]]
done
printf 'ok\n' >"$SIMA_SMOKE_BUNDLE/result"
kill -TERM 1
CLIENT

if ! timeout --kill-after=5s 25s env -u LD_LIBRARY_PATH -u LD_PRELOAD \
    SIMA_SMOKE_BUNDLE="$bundle" \
    unshare --user --map-root-user --pid --mount --fork --mount-proc --kill-child \
    bash -c '
        set -euo pipefail
        mount --make-rprivate /
        mount -t tmpfs tmpfs /etc
        mount -t tmpfs tmpfs /run
        mount -t tmpfs tmpfs /var/log
        cp "$SIMA_SMOKE_BUNDLE/client.sh" /etc/sima-smoke-client.sh
        printf "services:\n  - /etc/sima-smoke-client.yml\n  - /etc/sima-smoke-probe.yml\n" > /etc/sima.yml
        printf "name: shared-library-client\ncmdline: /bin/bash /etc/sima-smoke-client.sh\n" > /etc/sima-smoke-client.yml
        printf "name: shared-library-probe\ncmdline: /bin/sleep 60\n" > /etc/sima-smoke-probe.yml
        exec "$SIMA_SMOKE_BUNDLE/sima-init"
    ' >"$temporary_dir/init.log" 2>&1; then
    cat "$temporary_dir/init.log" >&2
    exit 1
fi

[[ "$(cat "$bundle/result")" == ok ]]
echo "Shared-library relocation, loader failure, and PID 1 IPC smoke tests passed."
