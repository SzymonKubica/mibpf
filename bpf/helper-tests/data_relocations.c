#include <stdint.h>
#include "../helpers.h"

const int c = 123;
const int d = 123;
const int *ptr = &c;

int test_relocation()
{
    bpf_printf("This is a test of c: %d\n", *ptr);
    return 0;
}
