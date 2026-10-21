export function Soon({ title, text }: { title: string; text: string }) {
  return (
    <div className="scroll">
      <div className="settings">
        <h1>{title}</h1>
        <p className="empty" style={{ paddingTop: 0 }}>{text}</p>
      </div>
    </div>
  );
}
