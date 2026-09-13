/* A foreign function, written against spec/09-prelude.md §9.8.1.
 *
 * It is here so the proof for stratum 8 calls something real rather than
 * something mocked. Three behaviours, which is every branch §9.8.1 has:
 * an answer that fits, an answer that does not, and a refusal. */

#include <stddef.h>
#include <stdint.h>
#include <string.h>

/* Echoes its arguments back, upper-cased. Fits, usually. */
int32_t said(const uint8_t *args, size_t args_len,
             uint8_t *out, size_t out_cap, size_t *out_len)
{
    *out_len = args_len;
    if (args_len > out_cap) return 1;
    for (size_t i = 0; i < args_len; i++) {
        uint8_t c = args[i];
        out[i] = (c >= 'a' && c <= 'z') ? (uint8_t)(c - 32) : c;
    }
    return 0;
}

/* Always needs more room than it was given, so the caller asks twice and
 * must not ask a third time. §9.8.1. */
int32_t greedy(const uint8_t *args, size_t args_len,
               uint8_t *out, size_t out_cap, size_t *out_len)
{
    (void)args; (void)args_len; (void)out;
    *out_len = out_cap + 1;
    return 1;
}

/* `3` is `denied`, in the order §5.1.1 lists them. */
int32_t forbidden(const uint8_t *args, size_t args_len,
                  uint8_t *out, size_t out_cap, size_t *out_len)
{
    (void)args; (void)args_len; (void)out; (void)out_cap;
    *out_len = 0;
    return 3;
}
