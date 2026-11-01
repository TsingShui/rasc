package fixtures;
public class Reader {
    public int value(int index) { return Dep.value(index); }
    private int secret(int index) { return Dep.value(index); }
    public int direct(int index) { return secret(index); }
}
