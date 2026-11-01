package fixtures;
public class SpecialReader extends Reader {
    @Override public int value(int index) { return super.value(index) + 3; }
}
