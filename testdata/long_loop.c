#include <stdio.h>

int main() {
  long long i = 0, x = 0;
  while (i < 5000000000) {
    x += i;
    i++;
  }
  printf("%lld\n", x);
}
