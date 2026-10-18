/* Observe the calling thread's native CPU scheduling priority through the
 * public Mach API. Built as both a ctypes library and an executed child by
 * the disposable launchd fixture; no policy is changed by this observer.
 */
#include <mach/mach.h>
#include <mach/thread_info.h>
#include <stdio.h>
#include <sys/resource.h>

int pigeon_base_priority(void) {
    thread_extended_info_data_t info;
    mach_msg_type_number_t count = THREAD_EXTENDED_INFO_COUNT;
    thread_t thread = mach_thread_self();
    kern_return_t result = thread_info(thread, THREAD_EXTENDED_INFO,
                                      (thread_info_t)&info, &count);
    mach_port_deallocate(mach_task_self(), thread);
    if (result != KERN_SUCCESS || count != THREAD_EXTENDED_INFO_COUNT) {
        return -1;
    }
    return info.pth_priority;
}

int main(void) {
    int priority = pigeon_base_priority();
    if (priority < 0) {
        fprintf(stderr, "could not inspect native thread scheduling\n");
        return 1;
    }
    printf("{\"base_priority\":%d,\"nice\":%d}\n", priority,
           getpriority(PRIO_PROCESS, 0));
    return 0;
}
