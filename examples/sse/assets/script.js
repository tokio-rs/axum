var eventSource = new EventSource("sse");

eventSource.onmessage = (event) => {
  console.log("Message from server ", event.data);
};
