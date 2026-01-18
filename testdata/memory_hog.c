#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main() {
    // Allocate 100MB
    size_t size = 100 * 1024 * 1024;
    char* ptr = malloc(size);
    if (ptr == NULL) {
        printf("malloc failed\n");
        return 1;
    }

    // Touch all pages to force allocation
    memset(ptr, 'x', size);

    printf("Allocated 100MB\n");
    return 0;
}
