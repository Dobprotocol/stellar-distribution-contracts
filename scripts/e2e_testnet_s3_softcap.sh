#!/usr/bin/env bash
#
# S-3 en TESTNET: una salida que rompe el soft cap tiene que hacer fracasar la
# campana, no dejarla en Succeeded para que el admin la active con menos plata
# de la prometida.
#
#   bash scripts/e2e_testnet_s3_softcap.sh
#
# Corre contra el wasm de crowdfunding YA instalado en testnet (el de la release
# cf-v0.3.0, el mismo hash que quedo en la tabla `networks`), asi que no sube
# nada ni recompila: solo despliega dos campanas nuevas y las lleva hasta el
# final. No toca mainnet.
#
# Reutiliza el TUSD y el splitter que dejo e2e_testnet_v2.sh en
# deploys_testnet_v2.json. El splitter hace de destino de la activacion: no se
# le distribuye nada aca, solo tiene que existir y saber responder
# `get_allocation`, que es lo que `propose_activation` sondea.
#
# Dos escenarios, que son los dos lados de la comparacion:
#   A) la salida deja la venta POR DEBAJO del minimo -> Failed, activate cerrado,
#      la propuesta retirada, y refund abierto de inmediato para quien se quedo
#      (sin esperar los 90 dias de expire_activation).
#   B) la salida cae EXACTAMENTE en el minimo -> la campana sigue viva y se
#      activa normal. Prueba que el corte es `<` y no `<=`.
#
# Nota de bash: igual que en e2e_testnet_v2.sh, nada de `pipefail` ni
# `cmd | grep -q` — grep sale antes, el CLI muere con SIGPIPE y la asercion
# termina reportando lo contrario de lo que paso.
set -u

NET=testnet
SRC=deployer
ADMIN=$(stellar keys address deployer)
SH1=$(stellar keys address sh1)
SH2=$(stellar keys address sh2)

# Release cf-v0.3.0, instalado en testnet y mainnet el 2026-08-06.
HASH_CF=595c713623b2da4adc688e40ee03264a229d21fe3d77d7d1e35cf424185d17eb

MANIFEST=deploys_testnet_v2.json
TUSD=$(sed -n 's/.*"rewardToken": *"\([^"]*\)".*/\1/p' $MANIFEST)
SPLITTER=$(sed -n 's/.*"splitter": *"\([^"]*\)".*/\1/p' $MANIFEST)
if [ -z "$TUSD" ] || [ -z "$SPLITTER" ]; then
    echo "no encontre rewardToken/splitter en $MANIFEST — corre antes e2e_testnet_v2.sh"
    exit 1
fi

PASS=0
FAIL=0
OUT=""
RES=""
ok()  { echo "  ✅ $1"; PASS=$((PASS + 1)); }
bad() { echo "  ❌ $1"; FAIL=$((FAIL + 1)); }
check() { if [ "$2" = "$3" ]; then ok "$1 ($2)"; else bad "$1: esperaba '$3', obtuve '$2'"; fi; }
tail3() { printf '%s' "$OUT" | tail -3 | tr '\n' ' '; }

run() {
    OUT=$(stellar contract invoke --id "$1" --source "$2" --network $NET -- "${@:3}" 2>&1)
    local st=$?
    RES=$(printf '%s' "$OUT" | tail -1)
    return $st
}

transient() { [[ "$OUT" == *"Request timeout"* || "$OUT" == *"error sending request"* || "$OUT" == *"Connection"* || "$OUT" == *"503"* ]]; }

must() {
    local label=$1; shift
    local i
    for i in 1 2 3; do
        if run "$@"; then ok "$label"; return; fi
        transient || break
        echo "  ...RPC intermitente en '$label', reintento $i"
        sleep 5
    done
    bad "$label — $(tail3)"
}

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

echo "admin:    $ADMIN"
echo "sh1:      $SH1"
echo "sh2:      $SH2"
echo "TUSD:     $TUSD"
echo "splitter: $SPLITTER"

# --------------------------------------------------------------- fondeo
echo
echo "[1] fondeando a los inversores"
must "mint a sh1" "$TUSD" $SRC mint --to "$SH1" --amount 100000
must "mint a sh2" "$TUSD" $SRC mint --to "$SH2" --amount 100000

# Las dos campanas comparten deadline para esperarlo una sola vez.
DEADLINE=$(( $(date +%s) + 150 ))

# ------------------------------------------------ A) la salida rompe el cap
echo
echo "[2] campana A — la salida deja la venta bajo el minimo"
CF_A=$(stellar contract deploy --wasm-hash "$HASH_CF" --source $SRC --network $NET 2>/dev/null | tail -1)
must "init A" "$CF_A" $SRC init \
    --admin "$ADMIN" --payment_token "$TUSD" \
    --price_per_share 100 --soft_cap_shares 100 --hard_cap_shares 1000 \
    --deadline "$DEADLINE" --payout_mode 0
echo "  campana A: $CF_A (soft cap 100 shares)"
must "sh1 aporta 70" "$CF_A" sh1 contribute --investor "$SH1" --shares_amount 70
must "sh2 aporta 50" "$CF_A" sh2 contribute --investor "$SH2" --shares_amount 50

# ------------------------------------------------ B) la salida cae justo
echo
echo "[3] campana B — la salida cae exactamente en el minimo"
CF_B=$(stellar contract deploy --wasm-hash "$HASH_CF" --source $SRC --network $NET 2>/dev/null | tail -1)
must "init B" "$CF_B" $SRC init \
    --admin "$ADMIN" --payment_token "$TUSD" \
    --price_per_share 100 --soft_cap_shares 100 --hard_cap_shares 1000 \
    --deadline "$DEADLINE" --payout_mode 0
echo "  campana B: $CF_B (soft cap 100 shares)"
must "sh1 aporta 100" "$CF_B" sh1 contribute --investor "$SH1" --shares_amount 100
must "sh2 aporta 20"  "$CF_B" sh2 contribute --investor "$SH2" --shares_amount 20

echo
echo "  esperando el deadline ($DEADLINE)..."
while [ "$(date +%s)" -le $((DEADLINE + 5)) ]; do sleep 5; done

# ------------------------------------------------------------ escenario A
echo
echo "[4] A: finalize -> propose -> sale sh1 -> la campana cae"
must "finalize A" "$CF_A" $SRC finalize
check "A queda Succeeded" "$RES" '1'
must "propose_activation A" "$CF_A" $SRC propose_activation --splitter_address "$SPLITTER"

run "$TUSD" $SRC balance --id "$SH1"; SH1_ANTES=${RES//\"/}
must "sh1 se sale" "$CF_A" sh1 opt_out --investor "$SH1"
check "le devolvieron su aporte entero" "${RES//\"/}" '7000'
run "$TUSD" $SRC balance --id "$SH1"
check "el dinero llego a la billetera de sh1" "$((${RES//\"/} - SH1_ANTES))" '7000'

# S-3: esto es lo que antes no pasaba. 120 - 70 = 50 < 100.
run "$CF_A" $SRC get_status
check "S-3 — A quedo Failed, no Succeeded" "$RES" '2'
run "$CF_A" $SRC get_total_raised
check "el escrow refleja solo lo que queda" "${RES//\"/}" '5000'

expect_err "S-3 — activate cerrado en una campana caida"  7 "$CF_A" $SRC activate --splitter_address "$SPLITTER"
expect_err "S-3 — tampoco se puede volver a proponer"     7 "$CF_A" $SRC propose_activation --splitter_address "$SPLITTER"
run "$CF_A" $SRC get_pending_activation
check "la propuesta se retiro con la salida" "$RES" 'null'

# Y el que se quedo cobra ya, sin esperar la ventana de 90 dias.
run "$TUSD" $SRC balance --id "$SH2"; SH2_ANTES=${RES//\"/}
must "sh2 se reembolsa de inmediato" "$CF_A" sh2 refund --investor "$SH2"
check "recupero su aporte" "${RES//\"/}" '5000'
run "$TUSD" $SRC balance --id "$SH2"
check "el dinero llego a la billetera de sh2" "$((${RES//\"/} - SH2_ANTES))" '5000'
run "$TUSD" $SRC balance --id "$CF_A"
check "la campana A quedo vacia" "${RES//\"/}" '0'

# ------------------------------------------------------------ escenario B
echo
echo "[5] B: la salida cae justo en el minimo y la campana sigue viva"
must "finalize B" "$CF_B" $SRC finalize
check "B queda Succeeded" "$RES" '1'
must "propose_activation B" "$CF_B" $SRC propose_activation --splitter_address "$SPLITTER"
must "sh2 se sale (20 shares)" "$CF_B" sh2 opt_out --investor "$SH2"
check "le devolvieron su aporte" "${RES//\"/}" '2000'

# 120 - 20 = 100, exactamente el minimo: el corte es `<`, no `<=`.
run "$CF_B" $SRC get_status
check "S-3 — B sigue Succeeded en el borde" "$RES" '1'

# La salida si retira la propuesta (esa parte de C-1 no cambia), asi que el
# admin vuelve a proponer contra la lista corregida y recien ahi cierra.
run "$CF_B" $SRC get_pending_activation
check "la salida retiro la propuesta igual" "$RES" 'null'
must "propose_activation B de nuevo" "$CF_B" $SRC propose_activation --splitter_address "$SPLITTER"

run "$TUSD" $SRC balance --id "$SPLITTER"; SPL_ANTES=${RES//\"/}
must "activate B" "$CF_B" $SRC activate --splitter_address "$SPLITTER"
run "$TUSD" $SRC balance --id "$SPLITTER"
check "el escrow que quedo llego al splitter" "$((${RES//\"/} - SPL_ANTES))" '10000'
run "$CF_B" $SRC get_status
check "B quedo Activated" "$RES" '3'

echo
echo "campana A (caida): $CF_A"
echo "campana B (viva):  $CF_B"
echo "resultado: $PASS ok / $FAIL fallidos"
[ "$FAIL" -eq 0 ] && echo "✅ E2E S-3 TESTNET OK" || { echo "❌ E2E con fallas"; exit 1; }
