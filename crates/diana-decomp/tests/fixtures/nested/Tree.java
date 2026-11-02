package fixtures;

public class Tree {
    private int n = 7;
    public Tree() {}
    public int read() { return n; }
    public static class A {
        public A() {}
        public static class Same {
            public Same() {}
            public int f() { return 1; }
        }
    }
    public static class B {
        public B() {}
        public static class Same {
            public Same() {}
            public int f() { return 2; }
        }
    }
}
