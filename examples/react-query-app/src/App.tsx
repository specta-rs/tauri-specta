import { FormEvent, useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { commands } from "./bindings";

export default function App() {
  const [name, setName] = useState("React Query");
  const [title, setTitle] = useState("");
  const [pendingDeleteId, setPendingDeleteId] = useState<number | null>(null);
  const queryClient = useQueryClient();

  const greetingQuery = useQuery(commands.greeting(name));
  const todosQuery = useQuery(commands.listTodos());

  const createTodoMutation = useMutation({
    ...commands.createTodo(title),
    onSuccess: () => {
      setTitle("");
      queryClient.invalidateQueries({ queryKey: ["list_todos"] });
    },
  });

  const deleteTodoMutation = useMutation({
    ...commands.deleteTodo(pendingDeleteId ?? 0),
    onSuccess: () => {
      setPendingDeleteId(null);
      queryClient.invalidateQueries({ queryKey: ["list_todos"] });
    },
  });

  useEffect(() => {
    if (pendingDeleteId === null) return;
    deleteTodoMutation.mutate();
  }, [pendingDeleteId]);

  function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!title.trim()) return;
    createTodoMutation.mutate();
  }

  return (
    <main className="page">
      <h1>React Query + Tauri Specta</h1>

      <section className="card">
        <h2>Generated Query</h2>
        <div className="row">
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Name"
          />
        </div>
        <p>
          {greetingQuery.isPending
            ? "Loading greeting..."
            : greetingQuery.data}
        </p>
      </section>

      <section className="card">
        <h2>Generated Mutations</h2>
        <form className="row" onSubmit={onSubmit}>
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="New todo title"
          />
          <button
            type="submit"
            disabled={createTodoMutation.isPending || !title.trim()}
          >
            {createTodoMutation.isPending ? "Adding..." : "Add Todo"}
          </button>
        </form>

        {todosQuery.isPending ? (
          <p>Loading todos...</p>
        ) : (todosQuery.data ?? []).length === 0 ? (
          <p>No todos yet.</p>
        ) : (
          <ul>
            {(todosQuery.data ?? []).map((todo) => (
              <li key={todo.id}>
                {todo.title}
                <button
                  type="button"
                  onClick={() => {
                    setPendingDeleteId(todo.id);
                  }}
                >
                  Delete
                </button>
              </li>
            ))}
          </ul>
        )}

        <p>
          Uses generated <code>queryOptions</code> and <code>mutationOptions</code>.
        </p>
      </section>
    </main>
  );
}
