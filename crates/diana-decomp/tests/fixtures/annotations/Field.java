package fixtures;

@Mark(value = "x", nums = {1, 2})
public final class Field {
    @Deprecated public int n;
    public int read() { return n; }
}
