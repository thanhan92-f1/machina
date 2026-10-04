#!/usr/bin/env bash
# VM network policy (CiliumNetworkPolicy schema), VM labels and packet-flow
# commands shared by machinactl (daemon, or the controller via --fleet) and
# platformctl (controller). Callers set NP_BASE (…/api/v1), NP_AUTH (curl
# args array) and NP_FLEET (1 when NP_BASE is the controller).

NP_BASE="${NP_BASE:-}"
NP_FLEET="${NP_FLEET:-0}"
if ! declare -p NP_AUTH &>/dev/null; then NP_AUTH=(); fi
NP_COLOR="${NP_COLOR:-auto}"

np_die() { echo "error: $*" >&2; exit 1; }

np_need() {
    command -v curl >/dev/null || np_die "curl is required"
    command -v jq >/dev/null || np_die "jq is required"
}

np_color_on() {
    case "$NP_COLOR" in
        always) return 0 ;;
        never) return 1 ;;
    esac
    [[ -t 1 && -z "${NO_COLOR:-}" && "${TERM:-}" != "dumb" ]]
}

# curl with auth; prints the body, fails with the server's error message on HTTP >= 400.
np_api() {
    local method=$1 path=$2
    shift 2
    local out code
    out=$(curl -sk -X "$method" "${NP_AUTH[@]}" -w $'\n%{http_code}' "$@" "${NP_BASE}${path}") || np_die "request to ${NP_BASE}${path} failed"
    code=${out##*$'\n'}
    out=${out%$'\n'*}
    if [[ "$code" -ge 400 || "$code" == "000" ]]; then
        local msg
        msg=$(jq -r '.error // .message // empty' <<<"$out" 2>/dev/null)
        echo "error: HTTP $code${msg:+: $msg}" >&2
        jq -r '(.errors // [])[] | "  \(.path): \(.message)"' <<<"$out" 2>/dev/null >&2
        exit 1
    fi
    printf '%s' "$out"
}

np_uri() { jq -rn --arg v "$1" '$v|@uri'; }

np_read_file() {
    local f=$1
    [[ -n "$f" ]] || np_die "missing -f FILE (use - for stdin)"
    if [[ "$f" == "-" ]]; then cat; else [[ -r "$f" ]] || np_die "cannot read $f"; cat "$f"; fi
}

np_print_issues() {
    jq -r '
      ((.errors // [])[] | "  \u001b[31merror\u001b[0m   \(.path): \(.message)"),
      ((.warnings // [])[] | "  \u001b[33mwarning\u001b[0m \(.path): \(.message)"),
      ((.compile_warnings // [])[] | "  \u001b[33mwarning\u001b[0m \(.)")' | np_strip
}

# Drop ANSI escapes when color is off.
np_strip() {
    if np_color_on; then cat; else sed $'s/\x1b\\[[0-9;]*m//g'; fi
}

# ── netpol ────────────────────────────────────────────────────────────────

np_netpol_usage() {
    cat <<'EOF'
Usage: netpol <command> [options]

  apply -f FILE|- [--dry-run]    Create/replace policies (CiliumNetworkPolicy /
                                 CiliumClusterwideNetworkPolicy / VmNetworkPolicy;
                                 multi-document YAML, JSON or kind: List)
  validate -f FILE|-             Schema check + preview of selected VMs and rules
  get [NAME] [-o yaml|json|wide] List policies, or show one
  delete NAME                    Delete a policy
  enable NAME | disable NAME     Toggle a policy (fleet only)
  test --from VM --to VM|IP|NAME [--port N] [--proto tcp|udp|sctp|icmp|any] [--icmp-type N]
       [--http-method M --path P --host H --header 'K: V'] [--sni NAME] [--dns-name NAME]
       [--kafka-api-key produce|fetch|… --kafka-topic T --kafka-client-id C --kafka-api-version N]
                                 Trace a connection (and an L7 request) through the
                                 policy set (like `cilium policy trace`)
  selectors                      Selector → matched VMs (like `cilium policy selectors`)
  endpoints                      VMs with identity, labels and enforcement
  status                         Sync state, enforcement mode, Cilium presence
  fqdn                           toFQDNs names learned from DNS replies to VMs
                                 (like `cilium fqdn cache list`)
  auth                           Mutual-authentication table (identity pairs,
                                 like `cilium-dbg auth list`)
  sync                           Push compiled policy to every host now (fleet only)
EOF
}

np_netpol_get() {
    local name="" out=""
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -o|--output) out="$2"; shift 2 ;;
            -o*) out="${1#-o}"; shift ;;
            *) name="$1"; shift ;;
        esac
    done
    if [[ -n "$name" ]]; then
        case "$out" in
            json) np_api GET "/vm-network-policies/$(np_uri "$name")" | jq . ;;
            *) np_api GET "/vm-network-policies/$(np_uri "$name")?format=yaml"; echo ;;
        esac
        return
    fi
    local body
    body=$(np_api GET /vm-network-policies)
    case "$out" in
        json) jq . <<<"$body" ;;
        yaml) jq -r '.items[] | "---", .yaml' <<<"$body" ;;
        *)
            {
                printf 'NAME\tKIND\tENABLED\tSELECTED\tDESCRIPTION\n'
                jq -r --arg wide "$out" '.items[] | [
                    .name, .kind, (if .enabled == false then "no" else "yes" end),
                    ((.selected_vms // []) | if $wide == "wide" then join(",") else (length|tostring) end),
                    (.description // "" | .[0:60])
                  ] | @tsv' <<<"$body"
            } | column -t -s $'\t'
            jq -r '(.warnings // [])[] | "warning: \(.)"' <<<"$body" >&2
            ;;
    esac
}

np_netpol_apply() {
    local file="" dry=0 validate_only=${NP_VALIDATE:-0}
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -f|--filename) file="$2"; shift 2 ;;
            --dry-run|--dry-run=*) dry=1; shift ;;
            *) np_die "unknown option: $1" ;;
        esac
    done
    local text body
    text=$(np_read_file "$file")
    if [[ "$validate_only" == 1 ]]; then
        body=$(curl -sk -X POST "${NP_AUTH[@]}" -H 'Content-Type: application/yaml' --data-binary @- \
            "${NP_BASE}/vm-network-policies/validate" <<<"$text") || np_die "request failed"
        dry=1
    elif [[ "$dry" == 1 ]]; then
        body=$(np_api POST "/vm-network-policies?dry_run=1" -H 'Content-Type: application/yaml' --data-binary @- <<<"$text")
    else
        body=$(np_api POST /vm-network-policies -H 'Content-Type: application/yaml' --data-binary @- <<<"$text")
    fi
    if [[ "$dry" == 1 ]]; then
        local valid
        valid=$(jq -r '.valid' <<<"$body")
        jq -r '.policies[]? | "policy/\(.name) (\(.kind)) selects \((.selected_vms // []) | length) VM(s): \((.selected_vms // []) | join(", "))"' <<<"$body"
        jq -r '"compiled rules: \(.rules // 0)"' <<<"$body"
        np_print_issues <<<"$body"
        if [[ "$valid" == "true" ]]; then echo "valid"; else echo "invalid" >&2; exit 1; fi
        return
    fi
    jq -r '.applied[]? | "vmnetworkpolicy/\(.) configured"' <<<"$body"
    np_print_issues <<<"$body"
    jq -r 'if .sync then (if .sync.skipped then "sync: skipped (\(.sync.skipped))" elif .sync.ok then "sync: ok — \(.sync.vms) VM(s), \(.sync.rules) rule(s), \(.sync.peers) peer(s)" else "sync: FAILED — \(.sync.error)" end) else empty end' <<<"$body"
}

np_netpol_test() {
    local from="" to="" port="" proto="tcp" icmp="" l7='{}'
    l7add() { l7=$(jq -c --arg k "$1" --arg v "$2" '. + {($k): $v}' <<<"$l7"); }
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --from|--src) from="$2"; shift 2 ;;
            --to|--dst) to="$2"; shift 2 ;;
            --port|--dport) port="$2"; shift 2 ;;
            --proto|--protocol) proto="$2"; shift 2 ;;
            --icmp-type) icmp="$2"; proto="icmp"; shift 2 ;;
            --http-method|--method) l7add http_method "$2"; shift 2 ;;
            --path|--http-path) l7add http_path "$2"; shift 2 ;;
            --host|--http-host) l7add http_host "$2"; shift 2 ;;
            --header|--http-header) l7=$(jq -c --arg v "$2" '.http_headers += [$v]' <<<"$l7"); shift 2 ;;
            --sni|--server-name) l7add server_name "$2"; shift 2 ;;
            --dns-name|--query) l7add dns_name "$2"; shift 2 ;;
            --kafka-api-key) l7add kafka_api_key "$2"; shift 2 ;;
            --kafka-topic) l7add kafka_topic "$2"; shift 2 ;;
            --kafka-client-id) l7add kafka_client_id "$2"; shift 2 ;;
            --kafka-api-version) l7=$(jq -c --argjson v "$2" '. + {kafka_api_version: $v}' <<<"$l7"); shift 2 ;;
            *) np_die "unknown option: $1" ;;
        esac
    done
    [[ -n "$from" && -n "$to" ]] || np_die "usage: netpol test --from VM|IP --to VM|IP [--port N] [--proto tcp]"
    local q body
    q=$(jq -n --arg f "$from" --arg t "$to" --arg p "$port" --arg pr "$proto" --arg i "$icmp" --argjson l7 "$l7" \
        '{from: $f, to: $t, protocol: $pr}
         + (if $p != "" then {port: ($p|tonumber)} else {} end)
         + (if $i != "" then {icmp_type: ($i|tonumber)} else {} end)
         + $l7')
    body=$(np_api POST /vm-network-policies/trace -H 'Content-Type: application/json' -d "$q")
    jq -r '
      def ep(e): "\(e.vm // e.input) \u001b[90m(\(e.kind), identity \(e.identity))\u001b[0m";
      def side(s; n): "\u001b[1m\(n)\u001b[0m  \(s.vm // "-")  \(
          if s.verdict == "allowed" then "\u001b[32mallowed\u001b[0m"
          elif s.verdict == "denied" or s.verdict == "default-deny" or s.verdict == "l7-denied" or s.verdict == "auth-failed" then "\u001b[31m\(s.verdict)\u001b[0m"
          else "\u001b[90m\(s.verdict)\u001b[0m" end)\(if s.enforced then "  \u001b[90m[default-deny]\u001b[0m" else "" end)",
        (if s.rule then "    ↳ \(s.rule)" else empty end),
        (if s.auth then "    \u001b[36m⚿ authentication: \(s.auth)\u001b[0m" else empty end),
        (if s.l7 then "    \u001b[35m◆ L7: \(s.l7)\u001b[0m" else empty end);
      "Tracing \(ep(.from)) → \(ep(.to))  \(.protocol | ascii_upcase)\(if .port > 0 then "/\(.port)" else "" end)",
      "",
      side(.egress; "Egress  at source      "),
      side(.ingress; "Ingress at destination "),
      "",
      (if .allowed then "Final verdict: \u001b[1;32m\(.summary)\u001b[0m"
       else "Final verdict: \u001b[1;31m\(.summary)\u001b[0m" end)' <<<"$body" | np_strip
}

np_netpol_selectors() {
    local body
    body=$(np_api GET /vm-network-policies/selectors)
    {
        printf 'POLICY\tPATH\tSELECTOR\tMATCHES\n'
        jq -r '.items[] | [.policy, .path, .selector, ((.vms // []) | if length == 0 then "(none)" else join(",") end)] | @tsv' <<<"$body"
    } | column -t -s $'\t'
}

np_netpol_endpoints() {
    local body
    body=$(np_api GET /vm-network-policies/endpoints)
    {
        printf 'VM\tIDENTITY\tHOST\tINGRESS\tEGRESS\tADDRESSES\tLABELS\n'
        jq -r '.items[] | [
            .name, (.identity|tostring), (.host // "-"),
            (if .ingress_enforced then "enforced" else "allow-all" end),
            (if .egress_enforced then "enforced" else "allow-all" end),
            ((.addresses // []) | join(",") | if . == "" then "-" else . end),
            ((.labels // {}) | to_entries | map("\(.key)=\(.value)") | join(","))
          ] | @tsv' <<<"$body"
    } | column -t -s $'\t'
}

np_netpol_status() {
    local body
    body=$(np_api GET /vm-network-policies/status)
    if [[ "${NP_JSON:-0}" == 1 ]]; then jq . <<<"$body"; return; fi
    jq -r '
      "Policies:     \(.policies // (.items|length? // 0))",
      (if .managed_by then "Managed by:   \(.managed_by)" else empty end),
      (if .enforcement then "Enforcement:  \(.enforcement.mode // "observe")\(if .enforcement.lease_remaining_secs then " (lease \(.enforcement.lease_remaining_secs)s)" else "" end)" else empty end),
      (if has("hosts") then empty elif .cilium then "Cilium:       present (\(.cilium)) — Cilium enforces its own endpoints; Machina enforces libvirt VMs" else "Cilium:       absent — Machina eBPF enforces natively" end),
      (if .last_sync then "Last sync:    \(.last_sync.at // "-")  \(if .last_sync.skipped then "skipped: \(.last_sync.skipped)" elif .last_sync.ok then "ok (\(.last_sync.vms) VMs, \(.last_sync.rules) rules, \(.last_sync.peers) peers)" else "FAILED: \(.last_sync.error)" end)" else empty end),
      (if .edge then "VM edge:      \(.edge.owner // "-") owner, \((.edge.taps // []) | length) tap(s), flow log \(if .edge.flow_log then "on" else "off" end)\(if (.edge.fqdn_rules // 0) > 0 then ", toFQDNs \(.edge.fqdn_rules) rule(s) / \(.edge.fqdn_cache // 0) learned address(es)" else "" end)\(if (.edge.l7_rules // 0) > 0 then ", L7 \(.edge.l7_rules) rule(s)" else "" end)\(if (.edge.auth_entries // 0) > 0 then ", \(.edge.auth_entries) authenticated pair(s)" else "" end)" else empty end),
      (if .edge.auth_cert then "Auth cert:    host \(.edge.auth_cert.host_id), expires \(.edge.auth_cert.not_after | todate)\(if .edge.auth_cert.listening then ", mTLS on :4250" else ", not listening" end)" else empty end),
      (if (.edge.proxy // "") != "" then "L7 proxy:     \(.edge.proxy)" else empty end),
      ((.hosts // [])[] | "  host \(.hostname // .host_id)  \(
          if .reachable == false then "\u001b[90munreachable\u001b[0m"
          elif .ok == false then "\u001b[31m\(.error // "error")\u001b[0m"
          elif .ok then "\u001b[32mok\u001b[0m" else "\u001b[90mnot synced\u001b[0m" end)  vms=\(.vms // 0) rules=\(.rules // 0) peers=\(.peers // 0) taps=\(.taps // 0) owner=\(.owner // "-")\(if .enforcing then " \u001b[1;31menforcing\u001b[0m" else " observe" end)\(if .cilium then " cilium=\(.cilium)" else "" end)  synced \(.synced_at // "-")")' <<<"$body" | np_strip
}

np_netpol_fqdn() {
    local body
    body=$(np_api GET /vm-network-policies/fqdn-cache)
    if [[ "${NP_JSON:-0}" == 1 ]]; then jq . <<<"$body"; return; fi
    {
        printf 'NAME\tADDRESS\tIDENTITY\tVM\tHOST\tTTL\tPATTERNS\n'
        jq -r '.items[] | [
            .name, .address, (if .identity == 0 then "-" else (.identity|tostring) end), (.vm // "-" | if . == "" then "-" else . end),
            (.hostname // "-"), "\(.expires_in_secs)s", ((.patterns // []) | join(","))
          ] | @tsv' <<<"$body"
    } | column -t -s $'\t'
}

np_netpol_auth() {
    local body
    body=$(np_api GET /vm-network-policies/auth)
    if [[ "${NP_JSON:-0}" == 1 ]]; then jq . <<<"$body"; return; fi
    {
        printf 'SUBJECT\tPEER\tMODE\tHOST\tEXPIRES\tSTATE\n'
        jq -r '.items[] | [
            "\(.subject) [\(.subject_identity)]", "\(.peer) [\(.peer_identity)]", .mode, (.hostname // "-"),
            (if .expires_in_secs > 0 then "\(.expires_in_secs)s" else "-" end), .state
          ] | @tsv' <<<"$body"
    } | column -t -s $'\t'
}

np_netpol_main() {
    np_need
    local sub="${1:-}"
    shift || true
    case "$sub" in
        apply|create|replace) np_netpol_apply "$@" ;;
        validate|lint) NP_VALIDATE=1 np_netpol_apply "$@" ;;
        get|ls|list) np_netpol_get "$@" ;;
        delete|rm) np_api DELETE "/vm-network-policies/$(np_uri "${1:?usage: netpol delete NAME}")" | jq -r '"vmnetworkpolicy/\(.deleted // .name) deleted"' ;;
        enable|disable)
            [[ "$NP_FLEET" == 1 ]] || np_die "enable/disable is a fleet (controller) feature; on a single host, delete and re-apply"
            local on=true; [[ "$sub" == disable ]] && on=false
            np_api PUT "/vm-network-policies/$(np_uri "${1:?usage: netpol $sub NAME}")/enabled" -H 'Content-Type: application/json' -d "{\"enabled\":$on}" >/dev/null
            echo "vmnetworkpolicy/$1 ${sub}d"
            ;;
        test|trace) np_netpol_test "$@" ;;
        selectors) np_netpol_selectors ;;
        endpoints|ep) np_netpol_endpoints ;;
        status) np_netpol_status ;;
        fqdn|fqdn-cache|dns) np_netpol_fqdn ;;
        auth) np_netpol_auth ;;
        sync)
            [[ "$NP_FLEET" == 1 ]] || np_die "sync is a fleet (controller) feature; the daemon resyncs on every change and every 60s"
            np_api POST /vm-network-policies/sync | jq .
            ;;
        observe) np_flow_main observe "$@" ;;
        ""|help|-h|--help) np_netpol_usage ;;
        *) np_die "unknown netpol command: $sub (try: netpol help)" ;;
    esac
}

# ── VM labels ─────────────────────────────────────────────────────────────

# Controller label routes take the VM id; accept a name and resolve it.
np_vm_ref() {
    local ref=$1
    if [[ "$NP_FLEET" != 1 || "$ref" =~ ^[0-9a-fA-F-]{36}$ ]]; then
        printf '%s' "$ref"
        return
    fi
    local id
    id=$(np_api GET /vms | jq -r --arg n "$ref" '(if type == "array" then . else (.items // .vms // []) end) | map(select(.name == $n)) | .[0].id // empty')
    [[ -n "$id" ]] || np_die "no VM named $ref"
    printf '%s' "$id"
}

np_label_main() {
    np_need
    local vm="${1:-}"
    [[ -n "$vm" ]] || { echo "usage: vm label VM [key=value ...] [key- ...]" >&2; exit 1; }
    shift
    local ref cur
    ref=$(np_vm_ref "$vm")
    cur=$(np_api GET "/vms/$(np_uri "$ref")/labels" | jq -c '.labels // {}')
    if [[ $# -eq 0 ]]; then
        jq -r 'to_entries[] | "\(.key)=\(.value)"' <<<"$cur"
        return
    fi
    local a
    for a in "$@"; do
        if [[ "$a" == *=* ]]; then
            cur=$(jq -c --arg k "${a%%=*}" --arg v "${a#*=}" '. + {($k): $v}' <<<"$cur")
        elif [[ "$a" == *- ]]; then
            cur=$(jq -c --arg k "${a%-}" 'del(.[$k])' <<<"$cur")
        else
            np_die "expected key=value or key-, got: $a"
        fi
    done
    np_api PUT "/vms/$(np_uri "$ref")/labels" -H 'Content-Type: application/json' -d "{\"labels\":$cur}" \
        | jq -r '.labels // {} | to_entries[] | "\(.key)=\(.value)"'
    echo "vm/$vm labeled" >&2
}

# ── flows ─────────────────────────────────────────────────────────────────

np_flow_usage() {
    cat <<'EOF'
Usage: flow <command> [filters]

  observe [-f|--follow] [--last N]   Packet flows, Hubble style (colors on a TTY;
                                     NO_COLOR or --color never to disable)
  top [--by pair|src|dst|port|policy|vm|drop|l7] [--limit N]
                                     Busiest flows over the recent window
  stats                              Verdict / protocol / direction / drop-reason
                                     breakdown with bar charts

Filters:
  --vm NAME  --from-vm NAME  --to-vm NAME  --label k=v  --ip ADDR  --cidr PREFIX
  --port N  --protocol tcp|udp|icmp|sctp  --verdict FORWARDED,DROPPED,AUDIT
  --drop-reason policy-deny|default-deny|l7-deny|auth-required|spoofed-source
  --policy SUBSTR  --direction ingress|egress
  --host NAME (fleet)  -o json|compact  --color always|never|auto
EOF
}

NP_FLOW_JQ_TSV='[
  .ts, (.host // ""), (.src_vm // ""), .src, (.src_port|tostring), (.src_identity|tostring),
  (.dst_vm // ""), .dst, (.dst_port|tostring), (.dst_identity|tostring), .proto,
  (if .icmp_type != null then "type=\(.icmp_type)" else (.tcp_flags // "") end),
  .verdict, .direction, (.drop_reason // ""), (.policy // ""), (.bytes|tostring), (.iface // ""),
  (if .l7 then "\(.l7_type // "l7"): \(.l7)" else "" end)
] | map(if . == "" then "-" else gsub("\t"; " ") end) | @tsv'

# TSV (see NP_FLOW_JQ_TSV) → one colored line per flow.
np_flow_render() {
    local c=0
    np_color_on && c=1
    awk -F'\t' -v c="$c" -v fleet="$NP_FLEET" '
    function col(code, s) { return c ? "\033[" code "m" s "\033[0m" : s }
    function ep(name, addr, port, id,   s) {
        s = (name != "-") ? col("1;36", name) col("90", "(" addr ")") : col("36", addr)
        if (port != "0" && port != "-") s = s col("90", ":") col("36", port)
        if (id != "0" && id != "-" && id != "2") s = s col("90", " [" id "]")
        else if (id == "2") s = s col("90", " [world]")
        return s
    }
    {
        ts = $1; t = ts
        if (match(ts, /T[0-9:]+(\.[0-9]+)?/)) { t = substr(ts, RSTART + 1, RLENGTH - 1); if (length(t) > 12) t = substr(t, 1, 12) }
        line = col("90", t) "  "
        if (fleet == 1 && $2 != "-") line = line col("35", "[" $2 "]") " "
        line = line ep($3, $4, $5, $6) col("1;37", " → ") ep($7, $8, $9, $10)
        proto = toupper($11); flags = ($12 == "-") ? "" : " " $12
        line = line "  " col("33", proto) col("90", flags)
        v = $13
        if (v == "FORWARDED") vv = col("1;32", "✔ FORWARDED")
        else if (v == "DROPPED") vv = col("1;31", "✘ DROPPED")
        else if (v == "AUDIT") vv = col("1;33", "◉ AUDIT")
        else vv = v
        line = line "  " vv
        if ($15 != "-") line = line col("31", " (" $15 ")")
        line = line "  " col("34", $14)
        if ($16 != "-") line = line "  " col("2;37", "↳ " $16)
        if ($19 != "-" && $19 != "") line = line "  " col("1;35", "◆ " $19)
        print line
        fflush()
    }'
}

np_flow_query() {
    local q=""
    local k v
    while [[ $# -gt 1 ]]; do
        k=$1 v=$2
        shift 2
        [[ -n "$v" ]] && q+="&${k}=$(np_uri "$v")"
    done
    printf '%s' "${q#&}"
}

np_flow_banner() {
    local what=$1 filters=$2
    np_color_on || { echo "# $what${filters:+ ($filters)}"; return; }
    printf '\033[1;37m●\033[0m \033[1m%s\033[0m' "$what"
    [[ -n "$filters" ]] && printf '  \033[90m%s\033[0m' "$filters"
    printf '\n\033[90m%s\033[0m\n' "TIME          SOURCE → DESTINATION                         PROTO  VERDICT  DIR  POLICY"
}

np_flow_main() {
    np_need
    local sub="${1:-observe}"
    case "$sub" in -*) sub=observe ;; *) shift || true ;; esac
    local follow=0 last=50 out="" by="pair" limit=15
    local vm="" from_vm="" to_vm="" label="" ip="" cidr="" port="" protocol="" verdict="" drop="" policy="" direction="" host=""
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -f|--follow) follow=1; shift ;;
            --last|-n) last="$2"; shift 2 ;;
            -o|--output) out="$2"; shift 2 ;;
            --by) by="$2"; shift 2 ;;
            --limit) limit="$2"; shift 2 ;;
            --vm) vm="$2"; shift 2 ;;
            --from-vm|--from) from_vm="$2"; shift 2 ;;
            --to-vm|--to) to_vm="$2"; shift 2 ;;
            --label) label="$2"; shift 2 ;;
            --ip) ip="$2"; shift 2 ;;
            --cidr) cidr="$2"; shift 2 ;;
            --port) port="$2"; shift 2 ;;
            --protocol|--proto) protocol="$2"; shift 2 ;;
            --verdict) verdict="$2"; shift 2 ;;
            --drop-reason) drop="$2"; shift 2 ;;
            --policy) policy="$2"; shift 2 ;;
            --direction) direction="$2"; shift 2 ;;
            --host) host="$2"; shift 2 ;;
            --color) NP_COLOR="$2"; shift 2 ;;
            --color=*) NP_COLOR="${1#--color=}"; shift ;;
            -h|--help) np_flow_usage; return ;;
            *) np_die "unknown option: $1 (try: flow help)" ;;
        esac
    done
    local fq
    fq=$(np_flow_query vm "$vm" from_vm "$from_vm" to_vm "$to_vm" label "$label" ip "$ip" cidr "$cidr" \
        port "$port" protocol "$protocol" verdict "$verdict" drop_reason "$drop" policy "$policy" \
        direction "$direction" host "$host")
    local filters
    filters=$(sed 's/&/ /g; s/%2C/,/g; s/%3D/=/g; s/%2F/\//g' <<<"$fq")

    case "$sub" in
        observe)
            if [[ "$follow" == 1 ]]; then
                [[ -z "$out" ]] && np_flow_banner "flow observe --follow" "$filters"
                local url="${NP_BASE}/flows/stream?last=${last}${fq:+&$fq}"
                local parse='select(startswith("data:")) | sub("^data: ?"; "") | select(length > 0) | fromjson'
                case "$out" in
                    json) curl -skN "${NP_AUTH[@]}" "$url" | jq -R --unbuffered -c "$parse" ;;
                    *) curl -skN "${NP_AUTH[@]}" "$url" | jq -R --unbuffered -r "$parse | $NP_FLOW_JQ_TSV" | np_flow_render ;;
                esac
            else
                local body
                body=$(np_api GET "/flows?limit=${last}${fq:+&$fq}")
                case "$out" in
                    json) jq -c '.items | reverse | .[]' <<<"$body" ;;
                    *)
                        np_flow_banner "flow observe" "$filters"
                        jq -r ".items | reverse | .[] | $NP_FLOW_JQ_TSV" <<<"$body" | np_flow_render
                        if [[ "$(jq '.items | length' <<<"$body")" == 0 ]]; then
                            echo "(no flows — is a VM network policy applied and the bpfd VM edge attached?)" >&2
                        fi
                        ;;
                esac
            fi
            ;;
        top) np_flow_top "$fq" "$by" "$limit" ;;
        stats) np_flow_stats "$fq" ;;
        help) np_flow_usage ;;
        *) np_die "unknown flow command: $sub (try: flow help)" ;;
    esac
}

# Horizontal bar: green forwarded, yellow audit, red dropped.
NP_BAR_AWK='
function col(code, s) { return c ? "\033[" code "m" s "\033[0m" : s }
function rep(ch, n,   s) { s = ""; while (n-- > 0) s = s ch; return s }
function bar(f, a, d, max, width,   wf, wa, wd) {
    if (max <= 0) return ""
    wf = int(f * width / max + 0.5); wa = int(a * width / max + 0.5); wd = int(d * width / max + 0.5)
    if (f > 0 && wf == 0) wf = 1; if (a > 0 && wa == 0) wa = 1; if (d > 0 && wd == 0) wd = 1
    return col("32", rep("█", wf)) col("33", rep("█", wa)) col("31", rep("█", wd))
}'

np_flow_top() {
    local fq=$1 by=$2 limit=$3 body c=0
    np_color_on && c=1
    body=$(np_api GET "/flows?limit=5000${fq:+&$fq}")
    local key
    case "$by" in
        pair) key='"\(.src_vm // .src) → \(.dst_vm // .dst):\(.dst_port)/\(.proto)"' ;;
        src) key='(.src_vm // .src)' ;;
        dst) key='"\(.dst_vm // .dst):\(.dst_port)/\(.proto)"' ;;
        port) key='"\(.dst_port)/\(.proto)"' ;;
        policy) key='(.policy // "(no policy)")' ;;
        vm) key='(.vm // "-")' ;;
        drop) key='(.drop_reason // "-")' ;;
        l7) key='(if .l7 then "\(.l7_type // "l7"): \(.l7)" else "(no L7)" end)' ;;
        *) np_die "--by must be pair|src|dst|port|policy|vm|drop|l7" ;;
    esac
    np_color_on && printf '\033[1m● flow top --by %s\033[0m  \033[90m%s\033[0m\n' "$by" "$(jq '.items|length' <<<"$body") flows" \
        || echo "# flow top --by $by"
    jq -r --argjson lim "$limit" ".items | group_by($key) | map({k: (.[0] | $key), n: length,
            f: (map(select(.verdict == \"FORWARDED\")) | length),
            a: (map(select(.verdict == \"AUDIT\")) | length),
            d: (map(select(.verdict == \"DROPPED\")) | length),
            b: (map(.bytes) | add)}) | sort_by(-.n) | .[:\$lim][] | [.k, .n, .f, .a, .d, .b] | @tsv" <<<"$body" |
    awk -F'\t' -v c="$c" "$NP_BAR_AWK"'
    { k[NR] = $1; n[NR] = $2; f[NR] = $3; a[NR] = $4; d[NR] = $5; b[NR] = $6; if ($2 > max) max = $2; if (length($1) > kw) kw = length($1) }
    END {
        if (NR == 0) { print "(no flows)"; exit }
        if (kw > 60) kw = 60
        printf "%s\n", col("90", sprintf("%-" kw "s  %6s  %6s  %6s  %6s  %s", "KEY", "FLOWS", "FWD", "AUDIT", "DROP", ""))
        for (i = 1; i <= NR; i++)
            printf "%-" kw "s  %6d  %s  %s  %s  %s\n", substr(k[i], 1, kw), n[i],
                col("32", sprintf("%6d", f[i])), col("33", sprintf("%6d", a[i])), col("31", sprintf("%6d", d[i])), bar(f[i], a[i], d[i], max, 30)
    }'
}

np_flow_stats() {
    local fq=$1 body c=0
    np_color_on && c=1
    body=$(np_api GET "/flows?limit=5000${fq:+&$fq}")
    jq -r '.items as $i |
      ("verdict", "proto", "direction", "drop_reason", "policy") as $f |
      ($i | group_by(.[$f] // "-") | map([$f, (.[0][$f] // "-"), length]) | sort_by(-.[2]) | .[:8][]) | @tsv' <<<"$body" |
    awk -F'\t' -v c="$c" -v total="$(jq '.items|length' <<<"$body")" "$NP_BAR_AWK"'
    { s[NR] = $1; k[NR] = $2; n[NR] = $3; if ($3 > max[$1]) max[$1] = $3 }
    END {
        printf "%s %s\n", col("1", "● flow stats"), col("90", total " flows (most recent window)")
        if (NR == 0) { print "(no flows)"; exit }
        for (i = 1; i <= NR; i++) {
            if (s[i] != prev) { h = s[i]; gsub(/_/, " ", h); printf "\n%s\n", col("1;37", toupper(substr(h, 1, 1)) substr(h, 2)); prev = s[i] }
            code = "36"
            if (k[i] == "FORWARDED") code = "32"; else if (k[i] == "DROPPED" || s[i] == "drop_reason" && k[i] != "-") code = "31"; else if (k[i] == "AUDIT") code = "33"
            pct = total > 0 ? n[i] * 100 / total : 0
            w = max[s[i]] > 0 ? int(n[i] * 30 / max[s[i]] + 0.5) : 0
            printf "  %-34s %6d %5.1f%%  %s\n", substr(k[i], 1, 34), n[i], pct, col(code, rep("█", w))
        }
    }'
}
