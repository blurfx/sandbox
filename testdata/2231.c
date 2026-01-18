#include <stdio.h>

int getDigit(int x){
	int count = 0;
	while(x){
		x /= 10;
		count++;
	}
	return count;
}

int nthMod(int n, int x){
	while(n--)
		x/=10;
	return x%10;
}
int main(){
	int i,j,d,n,sum;
	scanf("%d",&n);
	for(i=1;i<=n;i++){
		sum = i;
		d = getDigit(i);
		for(j=0;j<d;j++)
			sum += nthMod(j, i);
		if(sum == n) {
			sum = i;
			break;
		}
		sum = 0;
	}
	printf("%d",sum);
}
