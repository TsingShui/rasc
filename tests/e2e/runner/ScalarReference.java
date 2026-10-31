// B03 pure-oracle cross-check source, not a Diana fixture or production runtime.
public final class ScalarReference {
    public static int addI(int a,int b){return a+b;}
    public static int subI(int a,int b){return a-b;}
    public static int mulI(int a,int b){return a*b;}
    public static int divI(int a,int b){return a/b;}
    public static int remI(int a,int b){return a%b;}
    public static int andI(int a,int b){return a&b;}
    public static int orI(int a,int b){return a|b;}
    public static int xorI(int a,int b){return a^b;}
    public static int shlI(int a,int b){return a<<b;}
    public static int shrI(int a,int b){return a>>b;}
    public static int ushrI(int a,int b){return a>>>b;}
    public static long addL(long a,long b){return a+b;}
    public static long subL(long a,long b){return a-b;}
    public static long mulL(long a,long b){return a*b;}
    public static long divL(long a,long b){return a/b;}
    public static long remL(long a,long b){return a%b;}
    public static long andL(long a,long b){return a&b;}
    public static long orL(long a,long b){return a|b;}
    public static long xorL(long a,long b){return a^b;}
    public static long shlL(long a,int b){return a<<b;}
    public static long shrL(long a,int b){return a>>b;}
    public static long ushrL(long a,int b){return a>>>b;}
    public static int negI(int a){return -a;}
    public static int notI(int a){return ~a;}
    public static long negL(long a){return -a;}
    public static long notL(long a){return ~a;}
    public static int byteI(int a){return (byte)a;}
    public static int shortI(int a){return (short)a;}
    public static int charI(int a){return (char)a;}
    public static long widenI(int a){return (long)a;}
    public static int narrowL(long a){return (int)a;}
    public static int compareL(long a,long b){return a<b?-1:a==b?0:1;}
    public static int literalI(int a){return 7-a;}
    public static int constantI(){return 0x12345678;}
    public static long constantL(){return 0x123456789abcdef0L;}
}
