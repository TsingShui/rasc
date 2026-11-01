package fixtures;

public final class Driver {
    public static void main(String[] args) {
        RuntimeException error = new RuntimeException();
        Exception checked = new Exception();
        try { ThrowCase.nil(); } catch (NullPointerException caught) { System.out.println("null:NPE"); }
        try { ThrowCase.unchecked(error); } catch (RuntimeException caught) { System.out.println("unchecked:" + (caught == error)); }
        try { ThrowCase.unchecked(null); } catch (NullPointerException caught) { System.out.println("parameter:NPE"); }
        System.out.println("normal:" + ThrowCase.branch(false, error));
        try { ThrowCase.branch(true, error); } catch (RuntimeException caught) { System.out.println("branch:" + (caught == error)); }
        try { ThrowCase.checked(checked); } catch (Exception caught) { System.out.println("checked:" + (caught == checked)); }
    }
}
