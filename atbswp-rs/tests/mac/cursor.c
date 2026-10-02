/* Prints the current pointer position; needs no permission. Used to see
 * whether injected motion was honoured on a runner without Accessibility. */
#include <ApplicationServices/ApplicationServices.h>
#include <stdio.h>
int main(void)
{
	CGEventRef e = CGEventCreate(NULL);
	CGPoint p = CGEventGetLocation(e);
	printf("%d %d\n", (int)p.x, (int)p.y);
	CFRelease(e);
	return 0;
}
