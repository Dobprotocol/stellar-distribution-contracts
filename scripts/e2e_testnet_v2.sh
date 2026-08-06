#!/usr/bin/env bash
#
# End-to-end de la V2 en Stellar TESTNET: splitter_v2 + participation_token +
# crowdfunding_v1, con los fixes del scan de seguridad 2026-08 ejercitados
# contra una cadena real en vez de contra el entorno simulado de `cargo test`.
#
#   bash scripts/e2e_testnet_v2.sh
#
# Requiere: stellar CLI con la red `testnet` configurada y las identidades
# deployer / sh1 / sh2 / sh3 financiadas por friendbot. No toca mainnet ni
# gasta XLM real.
#
# Lo que NO se puede probar aca: los caminos que dependen de saltar el reloj
# (expiry de rondas, timelock de admin cumplido, expire_activation a los 90
# dias). Una cadena real no se puede warpear; esos caminos estan cubiertos con
# ledger.timestamp manipulado en las suites de cargo. El ciclo completo del
# crowdfunding SI se cierra aca, incluida la activacion, porque el timelock de
# activacion quedo en cero.
#
# Nota de bash, por si alguien la vuelve a pisar: aca NO se usa `pipefail` ni
# `cmd | grep -q`. `grep -q` sale apenas encuentra la coincidencia, el CLI
# muere con SIGPIPE, y con pipefail el pipeline devuelve 141 justo cuando la
# asercion se cumplio — o sea, el chequeo reporta lo contrario de lo que paso.
# Toda la salida se captura en una variable y se busca ahi.
set -u

NET=testnet
SRC=deployer
ADMIN=$(stellar keys address deployer)
SH1=$(stellar keys address sh1)
SH2=$(stellar keys address sh2)
SH3=$(stellar keys address sh3)

PASS=0
FAIL=0
OUT=""
RES=""
ok()  { echo "  ✅ $1"; PASS=$((PASS + 1)); }
bad() { echo "  ❌ $1"; FAIL=$((FAIL + 1)); }
check() { if [ "$2" = "$3" ]; then ok "$1 ($2)"; else bad "$1: esperaba '$3', obtuve '$2'"; fi; }
tail3() { printf '%s' "$OUT" | tail -3 | tr '\n' ' '; }

# run <id> <source> <fn> [args...] -> OUT = salida completa, RES = ultima linea
run() {
    OUT=$(stellar contract invoke --id "$1" --source "$2" --network $NET -- "${@:3}" 2>&1)
    local st=$?
    RES=$(printf '%s' "$OUT" | tail -1)
    return $st
}

# El RPC publico de testnet corta pedidos de vez en cuando ("Request timeout").
# Eso no dice nada del contrato, asi que se reintenta. Un timeout NO garantiza
# que la transaccion no haya aterrizado, de modo que en un reintento un error de
# "ya estaba hecho" (#2 AlreadyInitialized) cuenta como exito: significa que el
# intento anterior si entro.
transient() { [[ "$OUT" == *"Request timeout"* || "$OUT" == *"error sending request"* || "$OUT" == *"Connection"* || "$OUT" == *"503"* ]]; }

# Invocacion que tiene que funcionar; si no, se imprime el motivo.
must() {
    local label=$1; shift
    local i
    for i in 1 2 3; do
        if run "$@"; then ok "$label"; return; fi
        if [ $i -gt 1 ] && [[ "$OUT" == *"Error(Contract, #2)"* ]]; then
            ok "$label (ya estaba hecho: el intento anterior aterrizo pese al timeout)"
            return
        fi
        transient || break
        echo "  ...RPC intermitente en '$label', reintento $i"
        sleep 5
    done
    bad "$label — $(tail3)"
}

# Invocacion que tiene que fallar con un codigo de error del contrato.
expect_err() {
    local label=$1 code=$2; shift 2
    local i st=0
    for i in 1 2 3; do
        run "$@"; st=$?
        [ $st -eq 0 ] && break
        transient || break
        echo "  ...RPC intermitente en '$label', reintento $i"
        sleep 5
    done
    if [ $st -eq 0 ]; then
        bad "$label: paso sin error (esperaba #$code)"
    elif [[ "$OUT" == *"Error(Contract, #$code)"* ]]; then
        ok "$label -> Error #$code"
    else
        bad "$label: esperaba #$code — $(tail3)"
    fi
}

# panic_with_error / assert dentro del wasm: el CLI lo reporta como trampa.
expect_trap() {
    local label=$1; shift
    if run "$@"; then
        bad "$label: paso sin error (esperaba trampa)"
    elif [[ "$OUT" == *"UnreachableCodeReached"* || "$OUT" == *"Error("* ]]; then
        ok "$label rechazado"
    else
        bad "$label: no reventó — $(tail3)"
    fi
}

echo "admin: $ADMIN"
echo "sh1:   $SH1"
echo "sh2:   $SH2"

# ---------------------------------------------------------------- 1. upload
echo
echo "[1] subiendo wasms a testnet..."
W=target/wasm32v1-none/release
up() { stellar contract upload --wasm "$1" --source $SRC --network $NET 2>/dev/null | tail -1; }
HASH_SPLITTER=$(up $W/soro_splitter_v2.wasm)
HASH_PT=$(up $W/participation_token.wasm)
HASH_CF=$(up $W/crowdfunding_v1.wasm)
echo "  soro_splitter_v2:    $HASH_SPLITTER"
echo "  participation_token: $HASH_PT"
echo "  crowdfunding_v1:     $HASH_CF"

# ------------------------------------------------- 2. token de recompensa
# Una instancia de participation_token con el deployer como admin hace de
# token de premio / de pago: implementa la interfaz SEP-41 completa.
echo
echo "[2] token de recompensa (TUSD)..."
TUSD=$(stellar contract deploy --wasm-hash "$HASH_PT" --source $SRC --network $NET 2>/dev/null | tail -1)
must "TUSD initialize" "$TUSD" $SRC initialize --admin "$ADMIN" --decimal 0 --name TUSD --symbol TUSD
must "mint a admin"    "$TUSD" $SRC mint --to "$ADMIN" --amount 1000000
must "mint a sh1"      "$TUSD" $SRC mint --to "$SH1" --amount 1000000
echo "  TUSD: $TUSD"

# P-1: montos negativos. Antes del fix, mint(-x) dejaba el supply negativo y
# clawback(-x) EMITIA tokens; ahora ambos revientan en check_nonnegative_amount.
# Ojo con la sintaxis: el negativo va pegado con '=' o el CLI lo lee como otra
# bandera.
echo
echo "[3] P-1 — participation_token rechaza montos negativos"
run "$TUSD" $SRC total_supply; SUP_BEFORE=$RES
expect_trap "mint(-100)"     "$TUSD" $SRC mint --to "$SH2" --amount=-100
expect_trap "clawback(-100)" "$TUSD" $SRC clawback --from "$SH1" --amount=-100
run "$TUSD" $SRC total_supply
check "el supply no se movio" "$RES" "$SUP_BEFORE"

# ---------------------------------------------------- 4. splitter V2 + PT
# Orden obligatorio: el token de participacion tiene que tener como admin al
# splitter, asi que el splitter se despliega (sin init) primero.
echo
echo "[4] desplegando splitter V2 + su token de participacion..."
SPLITTER=$(stellar contract deploy --wasm-hash "$HASH_SPLITTER" --source $SRC --network $NET 2>/dev/null | tail -1)
PT=$(stellar contract deploy --wasm-hash "$HASH_PT" --source $SRC --network $NET 2>/dev/null | tail -1)
must "PT initialize (admin = splitter)" "$PT" $SRC initialize --admin "$SPLITTER" --decimal 0 --name POOL --symbol POOL
echo "  splitter: $SPLITTER"
echo "  pt:       $PT"

must "splitter init" "$SPLITTER" $SRC init \
    --admin "$ADMIN" \
    --shares "[{\"shareholder\":\"$SH1\",\"share\":\"7000\"},{\"shareholder\":\"$SH2\",\"share\":\"3000\"}]" \
    --mutable true \
    --participation_token "$PT"
run "$PT" $SRC total_supply
check "init acuño el supply de participacion" "$RES" '"10000"'

# El gate temporal por defecto son 12 h entre distribuciones, que en una corrida
# de un solo tiro bloquearia la segunda ronda. Se desactiva el intervalo pero se
# deja una ventana de reclamo valida (30 dias exactos, el piso de S-4).
echo
echo "[5] S-4 — piso de 30 dias en la ventana de reclamo"
expect_err "expiry = delay + 1s" 32 "$SPLITTER" $SRC set_distribution_config \
    --config '{"min_interval_seconds":0,"claim_delay_seconds":0,"round_expiry_seconds":1,"last_distribution_time":0}'
expect_err "ventana de 29 dias"  32 "$SPLITTER" $SRC set_distribution_config \
    --config '{"min_interval_seconds":0,"claim_delay_seconds":0,"round_expiry_seconds":2505600,"last_distribution_time":0}'
must "ventana de 30 dias exactos aceptada" "$SPLITTER" $SRC set_distribution_config \
    --config '{"min_interval_seconds":0,"claim_delay_seconds":0,"round_expiry_seconds":2592000,"last_distribution_time":0}'

# ----------------------------------------------------- 6. distribucion V2
echo
echo "[6] distribucion V2 (crear ronda -> claim -> doble claim)"
must "fondear el splitter" "$TUSD" $SRC mint --to "$SPLITTER" --amount 100000
must "create_distribution" "$SPLITTER" $SRC create_distribution --token_address "$TUSD"
echo "  ronda creada: $RES"
run "$SPLITTER" $SRC get_claimable --shareholder "$SH1" --round_id 0; CLAIMABLE1=$RES
run "$SPLITTER" $SRC get_claimable --shareholder "$SH2" --round_id 0; CLAIMABLE2=$RES
echo "  claimable sh1(70%): $CLAIMABLE1 | sh2(30%): $CLAIMABLE2"
run "$TUSD" $SRC balance --id "$SH1"; BEFORE=${RES//\"/}
must "sh1 claim" "$SPLITTER" sh1 claim --shareholder "$SH1" --round_id 0
run "$TUSD" $SRC balance --id "$SH1"; AFTER=${RES//\"/}
check "sh1 cobro lo que decia get_claimable" "\"$((AFTER - BEFORE))\"" "$CLAIMABLE1"

# S-1: el segundo claim de la misma ronda tiene que rebotar.
expect_err "S-1 — doble claim" 14 "$SPLITTER" sh1 claim --shareholder "$SH1" --round_id 0

# S-2: update_shares tiene que cuadrar contra el supply vivo del token.
echo
echo "[7] S-2 — update_shares se compara contra el supply real"
expect_err "lista que no cuadra con total_shares" 6 "$SPLITTER" $SRC update_shares \
    --shares "[{\"shareholder\":\"$SH1\",\"share\":\"5000\"}]"

# S-5: transferencia de admin en dos pasos.
echo
echo "[8] S-5 — set_admin no transfiere solo"
must "set_admin(sh3) propuesto" "$SPLITTER" $SRC set_admin --new_admin "$SH3"
run "$SPLITTER" $SRC get_config
if [[ "$OUT" == *"$ADMIN"* ]]; then
    ok "el admin sigue siendo el original hasta el accept_admin"
else
    bad "el admin cambio sin accept_admin: $RES"
fi
must "cancel_admin_transfer" "$SPLITTER" $SRC cancel_admin_transfer
expect_err "accept_admin tras cancelar" 3 "$SPLITTER" sh3 accept_admin

# S-6: get_claimable sobre una ronda de snapshot informa 0, no un monto que el
# claim normal jamas pagaria.
echo
echo "[9] S-6 — get_claimable sobre ronda de snapshot"
must "fondear para la ronda de snapshot" "$TUSD" $SRC mint --to "$SPLITTER" --amount 50000
ROOT=0000000000000000000000000000000000000000000000000000000000000001
must "create_distribution_snapshot" "$SPLITTER" $SRC create_distribution_snapshot \
    --token_address "$TUSD" --merkle_root "$ROOT"
run "$SPLITTER" $SRC get_claimable --shareholder "$SH1" --round_id 1
check "ronda de snapshot informa 0" "$RES" '"0"'
expect_err "claim normal sobre ronda de snapshot" 45 "$SPLITTER" sh1 claim --shareholder "$SH1" --round_id 1

# ------------------------------------------------------- 10. crowdfunding
echo
echo "[10] crowdfunding: init -> contribute -> finalize -> propose_activation"
CF=$(stellar contract deploy --wasm-hash "$HASH_CF" --source $SRC --network $NET 2>/dev/null | tail -1)
DEADLINE=$(( $(date +%s) + 120 ))
must "cf init" "$CF" $SRC init \
    --admin "$ADMIN" --payment_token "$TUSD" \
    --price_per_share 100 --soft_cap_shares 100 --hard_cap_shares 1000 \
    --deadline "$DEADLINE"
echo "  campaña: $CF (deadline $DEADLINE)"
must "sh1 contribuye 100 shares" "$CF" sh1 contribute --investor "$SH1" --shares_amount 100
run "$CF" $SRC get_total_raised
check "escrow recaudado" "$RES" '"10000"'

echo "  esperando el deadline..."
while [ "$(date +%s)" -le $((DEADLINE + 5)) ]; do sleep 5; done
must "finalize" "$CF" $SRC finalize
# El CLI devuelve el discriminante del enum, no su nombre: 1 = Succeeded.
check "estado Succeeded" "$RES" '1'

# C-1: la activacion es propose -> activate, en dos pasos y con el destino
# anunciado on-chain. El timelock quedo en cero a proposito, asi que el segundo
# paso puede seguir al primero de inmediato y una campana legitima no espera.
must "propose_activation" "$CF" $SRC propose_activation --splitter_address "$SPLITTER"
ETA=${RES//\"/}
echo "  activacion anunciada para: $(date -d @"$ETA" -Iseconds 2>/dev/null || echo "$ETA")"
expect_err "C-1 — activate a un destino distinto del propuesto" 19 "$CF" $SRC activate --splitter_address "$TUSD"
# propose_activation hace config.admin.require_auth(). En Soroban eso no
# devuelve Unauthorized: la transaccion ni siquiera se arma, porque exige una
# firma del admin que el invocador no tiene. El CLI lo dice explicitamente, y
# esa es justamente la prueba de que la guarda existe.
if run "$CF" sh2 propose_activation --splitter_address "$SPLITTER"; then
    bad "C-1 — un no-admin pudo proponer la activacion"
elif [[ "$OUT" == *"Missing signing key for account $ADMIN"* || "$OUT" == *"InvalidAction"* ]]; then
    ok "C-1 — propose_activation exige la firma del admin"
else
    bad "C-1 — propose_activation por un no-admin fallo por otro motivo — $(tail3)"
fi

# La otra mitad de C-1: mientras la propuesta este en pie el inversor puede
# irse con todo su dinero. Se simula (--send=no) para no vaciar el escrow ni
# retirar la propuesta; el camino real esta cubierto en las suites de cargo.
# La ventana de salida dura exactamente lo que el admin tarde en activar.
OUT=$(stellar contract invoke --id "$CF" --source sh1 --network $NET --send no -- \
        opt_out --investor "$SH1" 2>&1)
if [[ "$OUT" == *"10000"* ]]; then
    ok "C-1 — opt_out disponible mientras la propuesta esta en pie, devolveria 10000"
else
    bad "C-1 — opt_out NO esta disponible con una propuesta en pie — $(tail3)"
fi

# C-2: expire_activation todavia no aplica (la ventana sigue abierta). Va antes
# de activar, porque despues la campana ya no esta en Succeeded.
expect_err "C-2 — expire_activation antes de tiempo" 22 "$CF" sh2 expire_activation

# Y ahora se cierra la campana de verdad, cosa que el timelock de 7 dias hacia
# imposible en una sola corrida. El escrow completo se mueve al splitter.
# Ojo: $SPLITTER es el mismo del bloque de distribucion, o sea que ya tiene
# saldo. Lo que se mide es el delta, no el absoluto.
run "$TUSD" $SRC balance --id "$SPLITTER"; SPL_ANTES=${RES//\"/}
must "activate" "$CF" $SRC activate --splitter_address "$SPLITTER"
run "$TUSD" $SRC balance --id "$SPLITTER"; SPL_DESPUES=${RES//\"/}
check "el escrow entero llego al splitter" "$((SPL_DESPUES - SPL_ANTES))" '10000'
run "$TUSD" $SRC balance --id "$CF"
check "la campana quedo en cero" "${RES//\"/}" '0'
run "$CF" $SRC get_splitter
check "splitter registrado para el front/sync" "${RES//\"/}" "$SPLITTER"
# Cerrada la campana ya no hay de donde salirse.
expect_err "C-1 — opt_out despues de activar" 9 "$CF" sh1 opt_out --investor "$SH1"

# ------------------------------------------------------------- resultado
echo
cat > deploys_testnet_v2.json <<EOF
{
  "network": "testnet",
  "admin": "$ADMIN",
  "wasm": {
    "soro_splitter_v2": "$HASH_SPLITTER",
    "participation_token": "$HASH_PT",
    "crowdfunding_v1": "$HASH_CF"
  },
  "splitter": "$SPLITTER",
  "participationToken": "$PT",
  "rewardToken": "$TUSD",
  "crowdfunding": "$CF",
  "activationEta": $ETA
}
EOF
echo "manifiesto: deploys_testnet_v2.json"
echo "resultado: $PASS ok / $FAIL fallidos"
[ "$FAIL" -eq 0 ] && echo "✅ E2E STELLAR TESTNET OK" || { echo "❌ E2E con fallas"; exit 1; }
