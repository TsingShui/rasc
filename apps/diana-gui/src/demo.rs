//! Authored display content shared by the demo's navigator, search and reader.
//! These declarations, instructions and diagnostics are not Engine output.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClassId {
    NotesRepository,
    NoteStore,
    Note,
    NoteFilter,
    MainActivity,
    NotesAdapter,
    SyncWorker,
    SyncRequest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewMode {
    Java,
    Dex,
}

impl ViewMode {
    pub const ALL: [Self; 2] = [Self::Java, Self::Dex];
    pub fn label(self) -> &'static str {
        match self {
            Self::Java => "Java-like",
            Self::Dex => "DEX instructions",
        }
    }
}

/// A one-based line in authored UI content, not a DEX instruction coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceLine(pub u32);

pub struct Member {
    pub name: &'static str,
    pub line: SourceLine,
}

pub struct DemoClass {
    pub id: ClassId,
    pub name: &'static str,
    pub package: &'static str,
    pub members: &'static [Member],
    pub warning: Option<SourceLine>,
    java: &'static str,
}

pub const CLASSES: [&DemoClass; 8] = [
    &REPOSITORY,
    &STORE,
    &NOTE,
    &FILTER,
    &MAIN,
    &ADAPTER,
    &SYNC,
    &REQUEST,
];
pub const PACKAGES: [&str; 4] = [
    "demo.notes.data",
    "demo.notes.model",
    "demo.notes.ui",
    "demo.notes.sync",
];

impl ClassId {
    pub fn class(self) -> &'static DemoClass {
        match self {
            Self::NotesRepository => &REPOSITORY,
            Self::NoteStore => &STORE,
            Self::Note => &NOTE,
            Self::NoteFilter => &FILTER,
            Self::MainActivity => &MAIN,
            Self::NotesAdapter => &ADAPTER,
            Self::SyncWorker => &SYNC,
            Self::SyncRequest => &REQUEST,
        }
    }
    pub fn ordinal(self) -> usize {
        match self {
            Self::NotesRepository => 0,
            Self::NoteStore => 1,
            Self::Note => 2,
            Self::NoteFilter => 3,
            Self::MainActivity => 4,
            Self::NotesAdapter => 5,
            Self::SyncWorker => 6,
            Self::SyncRequest => 7,
        }
    }
    pub fn source(self, mode: ViewMode) -> &'static str {
        match mode {
            ViewMode::Java => self.class().java,
            ViewMode::Dex => DEMO_DEX,
        }
    }
}

static REPOSITORY: DemoClass = DemoClass {
    id: ClassId::NotesRepository,
    name: "NotesRepository",
    package: "demo.notes.data",
    members: &[
        Member {
            name: "NotesRepository(NoteStore, Clock)",
            line: SourceLine(13),
        },
        Member {
            name: "findAll()",
            line: SourceLine(18),
        },
        Member {
            name: "findById(long noteId)",
            line: SourceLine(22),
        },
        Member {
            name: "save(Note note)",
            line: SourceLine(26),
        },
        Member {
            name: "sync()",
            line: SourceLine(32),
        },
        Member {
            name: "delete(long noteId)",
            line: SourceLine(41),
        },
        Member {
            name: "setPinned(long, boolean)",
            line: SourceLine(47),
        },
        Member {
            name: "addObserver(Observer)",
            line: SourceLine(53),
        },
        Member {
            name: "removeObserver(Observer)",
            line: SourceLine(57),
        },
        Member {
            name: "notifyChanged(Note)",
            line: SourceLine(61),
        },
        Member {
            name: "countPending()",
            line: SourceLine(67),
        },
    ],
    warning: Some(SourceLine(35)),
    java: r#"package demo.notes.data;

import java.time.Clock;
import java.util.ArrayList;
import java.util.List;
import demo.notes.model.Note;

public final class NotesRepository {
    private final NoteStore store;
    private final Clock clock;
    private final List<Observer> observers = new ArrayList<>();

    public NotesRepository(NoteStore store, Clock clock) {
        this.store = store;
        this.clock = clock;
    }

    public List<Note> findAll() {
        return store.query("pinned DESC, updated_at DESC");
    }

    public Note findById(long noteId) {
        return store.find(noteId);
    }

    public void save(Note note) {
        note.setUpdatedAt(clock.millis());
        store.put(note);
        notifyChanged(note);
    }

    public void sync() {
        List<Note> pending = store.findPending();

        /* 此处保留原始调用，目标声明尚不完整。 */
        // Ldemo/notes/net/SyncApi;->push(Ljava/util/List;)V

        store.markSynced(pending);
    }

    public void delete(long noteId) {
        Note note = store.find(noteId);
        store.remove(noteId);
        notifyChanged(note);
    }

    public void setPinned(long noteId, boolean pinned) {
        Note note = store.find(noteId);
        note.setPinned(pinned);
        save(note);
    }

    public void addObserver(Observer observer) {
        observers.add(observer);
    }

    public void removeObserver(Observer observer) {
        observers.remove(observer);
    }

    private void notifyChanged(Note note) {
        for (Observer observer : observers) {
            observer.onNoteChanged(note);
        }
    }

    public int countPending() {
        return store.findPending().size();
    }
}
"#,
};

static STORE: DemoClass = DemoClass {
    id: ClassId::NoteStore,
    name: "NoteStore",
    package: "demo.notes.data",
    members: &[
        Member {
            name: "NoteStore(NoteDatabase)",
            line: SourceLine(6),
        },
        Member {
            name: "query(String orderBy)",
            line: SourceLine(10),
        },
        Member {
            name: "find(long noteId)",
            line: SourceLine(14),
        },
        Member {
            name: "put(Note note)",
            line: SourceLine(18),
        },
        Member {
            name: "remove(long noteId)",
            line: SourceLine(22),
        },
    ],
    warning: None,
    java: r#"package demo.notes.data;

public final class NoteStore {
    private final NoteDatabase database;

    public NoteStore(NoteDatabase database) {
        this.database = database;
    }

    public List<Note> query(String orderBy) {
        return database.query("notes", orderBy);
    }

    public Note find(long noteId) {
        return database.find("notes", noteId);
    }

    public void put(Note note) {
        database.upsert("notes", note.id, note);
    }

    public void remove(long noteId) {
        database.delete("notes", noteId);
    }
}
"#,
};

static NOTE: DemoClass = DemoClass {
    id: ClassId::Note,
    name: "Note",
    package: "demo.notes.model",
    members: &[
        Member {
            name: "Note(long id, String title)",
            line: SourceLine(8),
        },
        Member {
            name: "getTitle()",
            line: SourceLine(13),
        },
        Member {
            name: "isPinned()",
            line: SourceLine(17),
        },
    ],
    warning: None,
    java: r#"package demo.notes.model;

public final class Note {
    public final long id;
    private final String title;
    private boolean pinned;

    public Note(long id, String title) {
        this.id = id;
        this.title = title;
    }

    public String getTitle() {
        return title;
    }

    public boolean isPinned() {
        return pinned;
    }
}
"#,
};

static FILTER: DemoClass = DemoClass {
    id: ClassId::NoteFilter,
    name: "NoteFilter",
    package: "demo.notes.model",
    members: &[
        Member {
            name: "NoteFilter(String, boolean)",
            line: SourceLine(7),
        },
        Member {
            name: "matches(Note note)",
            line: SourceLine(12),
        },
        Member {
            name: "getQuery()",
            line: SourceLine(19),
        },
    ],
    warning: None,
    java: r#"package demo.notes.model;

public final class NoteFilter {
    private final String query;
    private final boolean pinnedOnly;

    public NoteFilter(String query, boolean pinnedOnly) {
        this.query = query;
        this.pinnedOnly = pinnedOnly;
    }

    public boolean matches(Note note) {
        if (pinnedOnly && !note.isPinned()) {
            return false;
        }
        return note.getTitle().contains(query);
    }

    public String getQuery() {
        return query;
    }
}
"#,
};

static MAIN: DemoClass = DemoClass {
    id: ClassId::MainActivity,
    name: "MainActivity",
    package: "demo.notes.ui",
    members: &[
        Member {
            name: "onCreate(Bundle state)",
            line: SourceLine(9),
        },
        Member {
            name: "showNotes()",
            line: SourceLine(17),
        },
        Member {
            name: "onNoteSelected(Note note)",
            line: SourceLine(23),
        },
    ],
    warning: None,
    java: r#"package demo.notes.ui;

import android.app.Activity;
import android.os.Bundle;
import demo.notes.model.Note;

public class MainActivity extends Activity {
    private NotesRepository repository;
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        setContentView(R.layout.activity_notes);
        repository = new NotesRepository();
        showNotes();
    }

    @Override
    private void showNotes() {
        List<Note> notes = repository.findAll();
        NotesAdapter adapter = new NotesAdapter(notes);
        adapter.setOnClickListener(this::onNoteSelected);
    }

    private void onNoteSelected(Note note) {
        Intent intent = new Intent(this, NoteActivity.class);
        intent.putExtra("note_id", note.id);
        startActivity(intent);
    }
}
"#,
};

static ADAPTER: DemoClass = DemoClass {
    id: ClassId::NotesAdapter,
    name: "NotesAdapter",
    package: "demo.notes.ui",
    members: &[
        Member {
            name: "NotesAdapter(List<Note>)",
            line: SourceLine(6),
        },
        Member {
            name: "getItemCount()",
            line: SourceLine(10),
        },
        Member {
            name: "bind(NoteView, int)",
            line: SourceLine(14),
        },
        Member {
            name: "getItemId(int)",
            line: SourceLine(20),
        },
    ],
    warning: None,
    java: r#"package demo.notes.ui;

public final class NotesAdapter {
    private final List<Note> notes;

    public NotesAdapter(List<Note> notes) {
        this.notes = notes;
    }

    public int getItemCount() {
        return notes.size();
    }

    public void bind(NoteView view, int position) {
        Note note = notes.get(position);
        view.setTitle(note.getTitle());
        view.setPinned(note.isPinned());
    }

    public long getItemId(int position) {
        return notes.get(position).id;
    }
}
"#,
};

static SYNC: DemoClass = DemoClass {
    id: ClassId::SyncWorker,
    name: "SyncWorker",
    package: "demo.notes.sync",
    members: &[
        Member {
            name: "doWork()",
            line: SourceLine(8),
        },
        Member {
            name: "schedule()",
            line: SourceLine(19),
        },
    ],
    warning: None,
    java: r#"package demo.notes.sync;

import java.io.IOException;

public class SyncWorker extends Worker {
    private final NotesRepository repository;

    public Result doWork() {
        try {
            repository.sync();
            return Result.success();
        } catch (IOException error) {
            return Result.retry();
        }
    }

    // 这里只演示阅读，不会执行任何调度。

    public static void schedule() {
        WorkManager.getInstance().enqueue(
            new OneTimeWorkRequest.Builder(SyncWorker.class).build()
        );
    }
}
"#,
};

static REQUEST: DemoClass = DemoClass {
    id: ClassId::SyncRequest,
    name: "SyncRequest",
    package: "demo.notes.sync",
    members: &[
        Member {
            name: "SyncRequest(List<Note>, long)",
            line: SourceLine(7),
        },
        Member {
            name: "getNotes()",
            line: SourceLine(12),
        },
        Member {
            name: "getCreatedAt()",
            line: SourceLine(16),
        },
        Member {
            name: "isEmpty()",
            line: SourceLine(20),
        },
    ],
    warning: None,
    java: r#"package demo.notes.sync;

public final class SyncRequest {
    private final List<Note> notes;
    private final long createdAt;

    public SyncRequest(List<Note> notes, long createdAt) {
        this.notes = notes;
        this.createdAt = createdAt;
    }

    public List<Note> getNotes() {
        return notes;
    }

    public long getCreatedAt() {
        return createdAt;
    }

    public boolean isEmpty() {
        return notes.isEmpty();
    }
}
"#,
};

const DEMO_DEX: &str = r#"; 手写 DEX 指令示意，与 Java-like 页不建立对应关系
; 未执行解码或分析

.method public example()V
.registers 2

0000    const/4          v0, 0x1
0001    const/4          v1, 0x2
0002    add-int/2addr    v0, v1
0003    return-void
.end method
"#;
